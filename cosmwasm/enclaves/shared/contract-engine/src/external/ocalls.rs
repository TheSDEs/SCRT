//! This file should be autogenerated based on the headers created from the .edl file.

use enclave_ffi_types::{Ctx, EnclaveBuffer, OcallReturn, UntrustedVmError, UserSpaceBuffer};
use sgx_types::*;

extern "C" {
    pub fn ocall_allocate(
        retval: *mut UserSpaceBuffer,
        buffer: *const u8,
        length: usize,
    ) -> sgx_status_t;

    pub fn ocall_read_db(
        retval: *mut OcallReturn,
        context: Ctx,
        vm_error: *mut UntrustedVmError,
        gas_used: *mut u64,
        block_height: u64,
        value: *mut EnclaveBuffer,
        proof: *mut EnclaveBuffer,
        mp_key: *mut EnclaveBuffer,
        key: *const u8,
        key_len: usize,
    ) -> sgx_status_t;

    pub fn ocall_query_chain(
        retval: *mut OcallReturn,
        context: Ctx,
        vm_error: *mut UntrustedVmError,
        gas_used: *mut u64,
        gas_limit: u64,
        value: *mut EnclaveBuffer,
        query: *const u8,
        query_len: usize,
        query_depth: u32,
    ) -> sgx_status_t;

    pub fn ocall_remove_db(
        retval: *mut OcallReturn,
        context: Ctx,
        vm_error: *mut UntrustedVmError,
        gas_used: *mut u64,
        key: *const u8,
        key_len: usize,
    ) -> sgx_status_t;

    pub fn ocall_write_db(
        retval: *mut OcallReturn,
        context: Ctx,
        vm_error: *mut UntrustedVmError,
        gas_used: *mut u64,
        key: *const u8,
        key_len: usize,
        value: *const u8,
        value_len: usize,
    ) -> sgx_status_t;

    pub fn ocall_multiple_write_db(
        retval: *mut OcallReturn,
        context: Ctx,
        vm_error: *mut UntrustedVmError,
        gas_used: *mut u64,
        keys: *const u8,
        keys_len: usize,
    ) -> sgx_status_t;
}
