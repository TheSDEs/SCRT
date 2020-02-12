PACKAGES=$(shell go list ./... | grep -v '/simulation')
VERSION := $(shell echo $(shell git describe --tags) | sed 's/^v//')
COMMIT := $(shell git log -1 --format='%H')
LEDGER_ENABLED ?= true
BINDIR ?= $(GOPATH)/bin

build_tags = 
ifeq ($(LEDGER_ENABLED),true)
  ifeq ($(OS),Windows_NT)
    GCCEXE = $(shell where gcc.exe 2> NUL)
    ifeq ($(GCCEXE),)
      $(error gcc.exe not installed for ledger support, please install or set LEDGER_ENABLED=false)
    else
      build_tags += ledger
    endif
  else
    UNAME_S = $(shell uname -s)
    ifeq ($(UNAME_S),OpenBSD)
      $(warning OpenBSD detected, disabling ledger support (https://github.com/cosmos/cosmos-sdk/issues/1988))
    else
      GCC = $(shell command -v gcc 2> /dev/null)
      ifeq ($(GCC),)
        $(error gcc not installed for ledger support, please install or set LEDGER_ENABLED=false)
      else
        build_tags += ledger
      endif
    endif
  endif
endif

ifeq ($(WITH_CLEVELDB),yes)
  build_tags += gcc
endif
build_tags += $(BUILD_TAGS)
build_tags := $(strip $(build_tags))

whitespace :=
whitespace += $(whitespace)
comma := ,
build_tags_comma_sep := $(subst $(whitespace),$(comma),$(build_tags))

ldflags = -X github.com/cosmos/cosmos-sdk/version.Name=Enigmachain \
	-X github.com/cosmos/cosmos-sdk/version.ServerName=engd \
	-X github.com/cosmos/cosmos-sdk/version.ClientName=engcli \
	-X github.com/cosmos/cosmos-sdk/version.Version=$(VERSION) \
	-X github.com/cosmos/cosmos-sdk/version.Commit=$(COMMIT) \
	-X "github.com/cosmos/cosmos-sdk/version.BuildTags=$(build_tags)"

ifeq ($(WITH_CLEVELDB),yes)
  ldflags += -X github.com/cosmos/cosmos-sdk/types.DBBackend=cleveldb
endif
ldflags += $(LDFLAGS)
ldflags := $(strip $(ldflags))

BUILD_FLAGS := -tags "$(build_tags)" -ldflags '$(ldflags)'

all: install
install: go.sum
		go install -mod=readonly $(BUILD_FLAGS) ./cmd/engd
		go install -mod=readonly $(BUILD_FLAGS) ./cmd/engcli

go.sum: go.mod
		@echo "--> Ensure dependencies have not been modified"
		GO111MODULE=on go mod verify

test:
	@go test -mod=readonly $(PACKAGES)

deb: install
		rm -rf /tmp/enigmachain
		mkdir -p /tmp/enigmachain/deb/bin
		mv "$(GOPATH)/bin/engcli" /tmp/enigmachain/deb/bin
		mv "$(GOPATH)/bin/engd" /tmp/enigmachain/deb/bin
		chmod +x /tmp/enigmachain/deb/bin/engd /tmp/enigmachain/deb/bin/engcli
		mkdir -p /tmp/enigmachain/deb/DEBIAN
		cp ./packaging/control /tmp/enigmachain/deb/DEBIAN/control
		echo "Version: 0.0.1" >> /tmp/enigmachain/deb/DEBIAN/control
		echo "" >> /tmp/enigmachain/deb/DEBIAN/control
		cp ./packaging/postinst /tmp/enigmachain/deb/DEBIAN/postinst
		chmod 755 /tmp/enigmachain/deb/DEBIAN/postinst
		cp ./packaging/postrm /tmp/enigmachain/deb/DEBIAN/postrm
		chmod 755 /tmp/enigmachain/deb/DEBIAN/postrm
		dpkg-deb --build /tmp/enigmachain/deb/ .
		rm -rf /tmp/enigmachain
