use attestation::sgx_quote::SgxQuoteStatus;
use attestation::sgx_report::AdvisoryIDs;
use log::{debug, error, info, trace, warn};
use secret_attestation_token::{
    AsAttestationToken, AttestationType, AuthenticationMaterialVerify, Error, FromAttestationToken,
    SecretAttestationToken, VerificationError,
};
// use ciborium::value::Value;
use crate::epid_quote::EpidSgxQuote;

use enclave_ffi_types::NodeAuthResult;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// AttestationReport can be signed by either the Intel Attestation Service
/// using EPID or Data Center Attestation Service (platform dependent) using ECDSA.
/// Or even something non-SGX
#[derive(Default, Serialize, Deserialize)]
pub struct EndorsedEpidAttestationReport {
    /// Attestation report generated by the hardware
    pub report: String,
    /// Singature of the report
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    pub signature: Vec<u8>,
    /// Certificate of the signer of the report
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    pub cert: Vec<u8>,
}

fn as_base64<S>(key: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&base64::encode(key))
}

fn from_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Base64Visitor;

    impl<'de> serde::de::Visitor<'de> for Base64Visitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "base64 ASCII text")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            base64::decode(v).map_err(E::custom)
        }
    }
    deserializer.deserialize_str(Base64Visitor)
}

/// A report that can be signed by Intel EPID (which generates
/// `EndorsedAttestationReport`) and then sent off of the platform to be
/// verified by remote client.
#[derive(Debug)]
pub struct ValidatedEpidAttestation {
    /// The freshness of the report, i.e., elapsed time after acquiring the
    /// report in seconds.
    // pub freshness: Duration,
    /// Quote status
    pub sgx_quote_status: attestation::sgx_quote::SgxQuoteStatus,
    /// Content of the quote
    pub sgx_quote_body: crate::epid_quote::EpidSgxQuote,
    pub platform_info_blob: Option<Vec<u8>>,
    pub advisory_ids: AdvisoryIDs,
}

impl EndorsedEpidAttestationReport {
    /// Validate the high level signing certificate. This checks that Intel signed the
    ///
    fn validate_report(&self) -> Result<ValidatedEpidAttestation, VerificationError> {
        // Verify report's signature - aka intel's signing cert
        let signing_cert = webpki::EndEntityCert::from(&self.cert).map_err(|_err| {
            error!("Failed to validate signature");
            VerificationError::ErrorGeneric
        })?;

        let (ias_cert, root_store) = crate::ias::get_ias_auth_config();

        let trust_anchors: Vec<webpki::TrustAnchor> = root_store
            .roots
            .iter()
            .map(|cert| cert.to_trust_anchor())
            .collect();

        let chain: Vec<&[u8]> = vec![&ias_cert];

        // set as 04.11.23(dd.mm.yy) - should be valid for the foreseeable future, and not rely on SystemTime
        let time_stamp = webpki::Time::from_seconds_since_unix_epoch(1_699_088_856);

        // note: there's no way to not validate the time, and we don't want to write this code
        // ourselves. We also can't just ignore the error message, since that means that the rest of
        // the validation didn't happen (time is validated early on)
        match signing_cert.verify_is_valid_tls_server_cert(
            attestation::sgx_quote::SUPPORTED_SIG_ALGS,
            &webpki::TLSServerTrustAnchors(&trust_anchors),
            &chain,
            time_stamp,
        ) {
            Ok(_) => info!("Certificate verified successfully"),
            Err(e) => {
                error!("Certificate verification error {:?}", e);
                return Err(VerificationError::ErrorGeneric);
            }
        };

        // Verify the signature against the signing cert
        match signing_cert.verify_signature(
            &webpki::RSA_PKCS1_2048_8192_SHA256,
            self.report.as_bytes(),
            &self.signature,
        ) {
            Ok(_) => info!("Signature verified successfully"),
            Err(e) => {
                warn!("Signature verification error {:?}", e);
                return Err(VerificationError::ErrorGeneric);
            }
        }

        ValidatedEpidAttestation::from_endorsed_report(self)
            .map_err(|_| VerificationError::ErrorGeneric)
    }
}

impl AsAttestationToken for EndorsedEpidAttestationReport {
    fn as_attestation_token(&self) -> SecretAttestationToken {
        let encoded = serde_json::to_vec(&self).unwrap();

        SecretAttestationToken {
            attestation_type: AttestationType::SgxEpid,
            data: encoded,
            node_key: Default::default(),
            block_info: Default::default(),
            signature: vec![],
            signing_cert: vec![],
        }
    }
}

impl FromAttestationToken<Self> for EndorsedEpidAttestationReport {
    fn from_attestation_token(other: &SecretAttestationToken) -> Self {
        serde_json::from_slice(&other.data).unwrap()
    }
}

impl AuthenticationMaterialVerify for EndorsedEpidAttestationReport {
    fn verify(&self) -> Result<enclave_crypto::NodeAuthPublicKey, VerificationError> {
        let validated = self
            .validate_report()
            .map_err(|_| VerificationError::ErrorGeneric)?;

        validated
            .verify()
            .map_err(|_| VerificationError::ErrorGeneric)?;

        let report_creator_public = validated.sgx_quote_body.isv_enclave_report;

        Ok(report_creator_public.get_owner_key())
    }
}

impl ValidatedEpidAttestation {
    /// Construct a AttestationReport from a X509 certificate and verify
    /// attestation report with the report_ca_cert which is from the attestation
    /// service provider.
    pub fn verify(&self) -> Result<(), NodeAuthResult> {
        verify_quote_status(self, None)?;

        self.sgx_quote_body.isv_enclave_report.verify()
    }

    pub fn from_endorsed_report(
        endorsed_report: &EndorsedEpidAttestationReport,
    ) -> Result<Self, Error> {
        // Verify and extract information from attestation report
        let attn_report: serde_json::Value = serde_json::from_str(&endorsed_report.report)?;
        trace!("attn_report: {}", attn_report);

        // Verify API version is supported
        let version = attn_report["version"]
            .as_u64()
            .ok_or(Error::ReportParseError)?;

        if version != 4 {
            warn!("API version incompatible");
            return Err(Error::ReportParseError);
        };

        let mut platform_info_blob = None;
        if let Some(blob) = attn_report["platformInfoBlob"].as_str() {
            let as_binary = hex::decode(blob).map_err(|_| {
                warn!("Error parsing platform info");
                Error::ReportParseError
            })?;
            platform_info_blob = Some(as_binary)
        }

        // Get quote status
        let sgx_quote_status = {
            let status_string = attn_report["isvEnclaveQuoteStatus"]
                .as_str()
                .ok_or_else(|| {
                    warn!("Error parsing enclave quote status");
                    Error::ReportParseError
                })?;
            attestation::sgx_quote::SgxQuoteStatus::from(status_string)
        };

        // Get quote body
        let sgx_quote_body = {
            let quote_encoded = attn_report["isvEnclaveQuoteBody"].as_str().ok_or_else(|| {
                warn!("Error unpacking enclave quote body");
                Error::ReportParseError
            })?;
            let quote_raw = base64::decode(&quote_encoded.as_bytes()).map_err(|_| {
                warn!("Error decoding encoded quote body");
                Error::ReportParseError
            })?;
            EpidSgxQuote::parse_from(quote_raw.as_slice()).map_err(|e| {
                warn!("Error parsing epid sgx quote");
                e
            })?
        };

        let advisories: Vec<String> = if let Some(raw) = attn_report.get("advisoryIDs") {
            serde_json::from_value(raw.clone()).map_err(|_| {
                warn!("Failed to decode advisories");
                Error::ReportParseError
            })?
        } else {
            vec![]
        };

        // We don't actually validate the public key, since we use ephemeral certificates,
        // and all we really care about that the report is valid and the key that is saved in the
        // report_data field

        debug!("ValidatedEpidAttestation created successfully");

        Ok(Self {
            sgx_quote_status,
            sgx_quote_body,
            platform_info_blob,
            advisory_ids: attestation::sgx_report::AdvisoryIDs(advisories),
        })
    }
}

#[cfg(all(feature = "SGX_MODE_HW", feature = "production"))]
pub fn _verify_quote_status(
    report: &ValidatedEpidAttestation,
    _extra_advisories: Option<&AdvisoryIDs>,
) -> Result<NodeAuthResult, NodeAuthResult> {
    // info!(
    //     "Got GID: {:?}",
    //     transform_u32_to_array_of_u8(report.sgx_quote_body.gid)
    // );

    // if !check_epid_gid_is_whitelisted(&report.sgx_quote_body.gid) {
    //     error!(
    //         "Platform verification error: quote status {:?}",
    //         &report.sgx_quote_body.gid
    //     );
    //     return Err(NodeAuthResult::BadQuoteStatus);
    // }

    match &report.sgx_quote_status {
        SgxQuoteStatus::OK
        | SgxQuoteStatus::SwHardeningNeeded
        | SgxQuoteStatus::ConfigurationAndSwHardeningNeeded => {
            attestation::sgx_quote::check_advisories(
                &report.sgx_quote_status,
                &report.advisory_ids,
            )?;

            Ok(NodeAuthResult::Success)
        }
        _ => {
            error!(
                "Invalid attestation quote status - cannot verify remote node: {:?}",
                &report.sgx_quote_status
            );
            Err(NodeAuthResult::from(&report.sgx_quote_status))
        }
    }
}

// the difference here is that we allow GROUP_OUT_OF_DATE for testnet machines to make joining a bit
// easier
#[cfg(all(feature = "SGX_MODE_HW", not(feature = "production")))]
pub fn verify_quote_status(
    report: &ValidatedEpidAttestation,
    _extra_advisories: Option<&AdvisoryIDs>,
) -> Result<NodeAuthResult, NodeAuthResult> {
    match &report.sgx_quote_status {
        SgxQuoteStatus::OK
        | SgxQuoteStatus::SwHardeningNeeded
        | SgxQuoteStatus::ConfigurationAndSwHardeningNeeded
        | SgxQuoteStatus::GroupOutOfDate => {
            let results = attestation::sgx_quote::check_advisories(
                &report.sgx_quote_status,
                &report.advisory_ids,
            );

            if let Err(results) = results {
                warn!("This platform has vulnerabilities that will not be approved on mainnet");
                return Ok(results); // Allow in non-production
            }

            // if !advisories.contains_lvi_injection() {
            //     return Err(NodeAuthResult::EnclaveQuoteStatus);
            // }

            Ok(NodeAuthResult::Success)
        }
        _ => {
            error!(
                "Invalid attestation quote status - cannot verify remote node: {:?}",
                &report.sgx_quote_status
            );
            Err(NodeAuthResult::from(&report.sgx_quote_status))
        }
    }
}
