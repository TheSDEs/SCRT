package keeper

import (
	"cosmossdk.io/store/prefix"
	"github.com/cosmos/cosmos-sdk/runtime"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/scrtlabs/SecretNetwork/x/registration/internal/types"
	ra "github.com/scrtlabs/SecretNetwork/x/registration/remote_attestation"
)

func (k Keeper) GetMasterKey(ctx sdk.Context, keyType string) *types.MasterKey {
	store := k.storeService.OpenKVStore(ctx)
	var key types.MasterKey
	certBz, _ := store.Get(types.MasterKeyPrefix(keyType))
	if certBz == nil {
		return nil
	}
	k.cdc.MustUnmarshal(certBz, &key)

	return &key
}

func (k Keeper) SetMasterKey(ctx sdk.Context, key types.MasterKey, keyType string) {
	store := k.storeService.OpenKVStore(ctx)

	store.Set(types.MasterKeyPrefix(keyType), k.cdc.MustMarshal(&key))
}

func (k Keeper) isMasterCertificateDefined(ctx sdk.Context, keyType string) bool {
	regInfo := k.GetMasterKey(ctx, keyType)
	return regInfo != nil
}

func (k Keeper) getRegistrationInfo(ctx sdk.Context, publicKey types.NodeID) *types.RegistrationNodeInfo {
	store := k.storeService.OpenKVStore(ctx)
	var nodeInfo types.RegistrationNodeInfo
	// fmt.Println("pubkey", hex.EncodeToString(publicKey))
	certBz, _ := store.Get(types.RegistrationKeyPrefix(publicKey))

	if certBz == nil {
		return nil
	}
	k.cdc.MustUnmarshal(certBz, &nodeInfo)

	return &nodeInfo
}

func (k Keeper) ListRegistrationInfo(ctx sdk.Context, cb func([]byte, types.RegistrationNodeInfo) bool) {
	prefixStore := prefix.NewStore(runtime.KVStoreAdapter(k.storeService.OpenKVStore(ctx)), types.RegistrationStorePrefix)
	iter := prefixStore.Iterator(nil, nil)
	for ; iter.Valid(); iter.Next() {
		var regInfo types.RegistrationNodeInfo
		k.cdc.MustUnmarshal(iter.Value(), &regInfo)
		// cb returns true to stop early
		if cb(iter.Key(), regInfo) {
			break
		}
	}
}

func (k Keeper) SetRegistrationInfo(ctx sdk.Context, certificate types.RegistrationNodeInfo) {
	store := k.storeService.OpenKVStore(ctx)

	publicKey, err := ra.VerifyRaCert(certificate.Certificate)
	if err != nil {
		return
	}

	// fmt.Println("pubkey", hex.EncodeToString(publicKey))
	// fmt.Println("EncryptedSeed", hex.EncodeToString(certificate.EncryptedSeed))
	store.Set(types.RegistrationKeyPrefix(publicKey), k.cdc.MustMarshal(&certificate))
}

func (k Keeper) isNodeAuthenticated(ctx sdk.Context, publicKey types.NodeID) (bool, error) {
	regInfo := k.getRegistrationInfo(ctx, publicKey)
	if regInfo == nil {
		return false, nil
	}

	if regInfo.EncryptedSeed == nil {
		return false, nil
	}
	return true, nil
}
