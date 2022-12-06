package v120

import (
	v120registration "github.com/scrtlabs/SecretNetwork/x/registration/internal/types"
	v106registration "github.com/scrtlabs/SecretNetwork/x/registration/legacy/v106"
)

// Migrate accepts exported v1.0.6 x/registration genesis state and
// migrates it to v1.2.0 x/registration genesis state. The migration includes:
//
// - Re-encode in v1.2.0 GenesisState.
func Migrate(regGenState v106registration.GenesisState) *v120registration.GenesisState {
	registrations := make([]*v120registration.RegistrationNodeInfo, len(regGenState.Registration))
	for i, regNodeInfo := range regGenState.Registration {
		registrations[i] = &v120registration.RegistrationNodeInfo{
			Certificate:   v120ra.Certificate(regNodeInfo.Certificate),
			EncryptedSeed: regNodeInfo.EncryptedSeed,
		}
	}

	return &v120registration.GenesisState{
		Registration: registrations,
		NodeExchMasterKey: &v120registration.MasterKey{
			Bytes: regGenState.NodeExchMasterCertificate,
		},
		IoMasterKey: &v120registration.MasterKey{
			Bytes: regGenState.IoMasterCertificate,
		},
	}
}
