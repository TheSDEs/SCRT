package mock

// To be able to run unit tests without needing the enclave

type MockEnclaveApi struct{} //nolint:revive

func (MockEnclaveApi) LoadSeed(_ []byte, _ []byte, _ []byte) (bool, error) {
	return true, nil
}

func (MockEnclaveApi) GetEncryptedSeed(_ []byte) ([]byte, error) {
	return []byte(""), nil
}

func (MockEnclaveApi) GetEncryptedGenesisSeed(_ []byte) ([]byte, error) {
	return []byte(""), nil
}
