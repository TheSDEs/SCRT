use sgx_types::{
    sgx_epid_group_id_t, sgx_quote_nonce_t, sgx_quote_sign_type_t, sgx_report_t, sgx_spid_t,
    sgx_status_t, sgx_target_info_t, sgx_ql_qe_report_info_t, sgx_ql_qv_result_t
};

extern "C" {
    pub fn ocall_sgx_init_quote(
        ret_val: *mut sgx_status_t,
        ret_ti: *mut sgx_target_info_t,
        ret_gid: *mut sgx_epid_group_id_t,
    ) -> sgx_status_t;
    pub fn ocall_get_ias_socket(ret_val: *mut sgx_status_t, ret_fd: *mut i32) -> sgx_status_t;
    #[allow(dead_code)]
    pub fn ocall_get_sn_tss_socket(ret_val: *mut sgx_status_t, ret_fd: *mut i32) -> sgx_status_t;
    pub fn ocall_get_quote(
        ret_val: *mut sgx_status_t,
        p_sigrl: *const u8,
        sigrl_len: u32,
        p_report: *const sgx_report_t,
        quote_type: sgx_quote_sign_type_t,
        p_spid: *const sgx_spid_t,
        p_nonce: *const sgx_quote_nonce_t,
        p_qe_report: *mut sgx_report_t,
        p_quote: *mut u8,
        maxlen: u32,
        p_quote_len: *mut u32,
    ) -> sgx_status_t;
    pub fn ocall_get_quote_ecdsa_params(
        ret_val: *mut sgx_status_t,
        p_qe_info: *mut sgx_target_info_t,
        p_quote_size: *mut u32,
    ) -> sgx_status_t;
    pub fn ocall_get_quote_ecdsa(
        ret_val: *mut sgx_status_t,
        p_report: *const sgx_report_t,
        p_quote: *mut u8,
        n_quote: u32,
    ) -> sgx_status_t;
    pub fn ocall_get_quote_ecdsa_collateral(
        ret_val: *mut sgx_status_t,
        p_quote: *const u8,
        n_quote: u32,
        p_col: *mut u8,
        n_col: u32,
        p_col_out: *mut u32,
    ) -> sgx_status_t;
    pub fn ocall_verify_quote_ecdsa(
        ret_val: *mut sgx_status_t,
        p_quote: *const u8,
        n_quote: u32,
        p_col: *const u8,
        n_col: u32,
        p_target_info: *const sgx_target_info_t,
        time_s: i64,
        p_qve_report_info: *mut sgx_ql_qe_report_info_t,
        p_supp_data: *mut u8,
        n_supp_data: u32,
        p_supp_data_size: *mut u32,
        p_time_s: *mut i64,
        p_collateral_expiration_status: *mut u32,
        p_qv_result: *mut sgx_ql_qv_result_t,
    ) -> sgx_status_t;
}
