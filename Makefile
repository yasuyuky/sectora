RUST_VER=$(shell cat rust-toolchain)
VERSION=$(shell grep "^version" Cargo.toml | cut -f 2 -d '"')
X64_TARGET=x86_64-unknown-linux-gnu
ARM_TARGET=arm-unknown-linux-gnueabihf
X64_RELEASE_DIR=target/$(X64_TARGET)/release
ARM_RELEASE_DIR=target/$(ARM_TARGET)/release
X64_DEBIAN_DIR=target/$(X64_TARGET)/debian
ARM_DEBIAN_DIR=target/$(ARM_TARGET)/debian
X64_BUILD_IMG=ghcr.io/yasuyuky/rust-ubuntu:${RUST_VER}
ARM_BUILD_IMG=ghcr.io/yasuyuky/rust-arm:${RUST_VER}
COMMON_BUILD_OPT= -v ${PWD}:/source -w /source
LOG_LEVEL:=OFF
OPENSSL_STATIC_OPT= -e OPENSSL_STATIC=yes -e OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu/ -e OPENSSL_INCLUDE_DIR=/usr/include -e LOG_LEVEL=$(LOG_LEVEL)
X64_BUILD_VOL= -v ${PWD}/.cargo-x64/registry:/usr/local/cargo/registry -v ${PWD}/.cargo-x64/bin:/source/.cargo/bin
ARM_BUILD_VOL= -v ${PWD}/.cargo-arm/registry:/usr/local/cargo/registry -v ${PWD}/.cargo-arm/bin:/source/.cargo/bin
X64_BUILD_OPT= $(X64_BUILD_VOL) $(COMMON_BUILD_OPT) $(OPENSSL_STATIC_OPT)
ARM_BUILD_OPT= $(ARM_BUILD_VOL) $(COMMON_BUILD_OPT)
DEPLOY_TEST_IMG=yasuyuky/ubuntu-ssh
ENTRIY_POINTS := src/main.rs src/daemon.rs src/lib.rs
SRCS := $(filter-out $(ENTRIY_POINTS),$(wildcard src/*.rs))
ASSETS := $(wildcard assets/*) $(wildcard assets/*/*)
CARGO_FILES := Cargo.toml Cargo.lock rust-toolchain
DOCKER_RUN=docker run --rm

all: x64 arm deb

deb: x64-deb arm-deb

x64: x64-exe x64-daemon x64-lib

x64-exe: $(X64_RELEASE_DIR)/sectora

x64-daemon: $(X64_RELEASE_DIR)/sectorad

x64-lib: $(X64_RELEASE_DIR)/libnss_sectora.so

x64-deb: $(X64_DEBIAN_DIR)/sectora_$(VERSION)_amd64.deb

arm: arm-exe arm-daemon arm-lib

arm-exe: $(ARM_RELEASE_DIR)/sectora

arm-daemon: $(X64_RELEASE_DIR)/sectorad

arm-lib: $(ARM_RELEASE_DIR)/libnss_sectora.so

arm-deb: $(ARM_DEBIAN_DIR)/sectora_$(VERSION)_armhf.deb

enter-build-image:
	$(DOCKER_RUN) -it $(X64_BUILD_OPT) $(X64_BUILD_IMG) bash

$(X64_RELEASE_DIR)/sectora: src/main.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo build --bin sectora --release --target=$(X64_TARGET)

$(X64_RELEASE_DIR)/sectorad: src/daemon.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo build --bin sectorad --release --target=$(X64_TARGET)

$(X64_RELEASE_DIR)/libnss_sectora.so: src/lib.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo build --lib --release --target=$(X64_TARGET)

$(X64_DEBIAN_DIR)/sectora_$(VERSION)_amd64.deb: src/main.rs src/daemon.rs src/lib.rs $(SRCS) $(CARGO_FILES) $(ASSETS)
	$(DOCKER_RUN) $(X64_BUILD_OPT) $(X64_BUILD_IMG) sh -c "cargo install -f cargo-deb --root .cargo && CARGO_HOME=.cargo cargo deb --target=$(X64_TARGET)"

$(ARM_RELEASE_DIR)/sectora: src/main.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo build --bin sectora --release --target=$(ARM_TARGET)

$(ARM_RELEASE_DIR)/sectorad: src/daemon.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo build --bin sectorad --release --target=$(ARM_TARGET)

$(ARM_RELEASE_DIR)/libnss_sectora.so: src/lib.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo build --lib --release --target=$(ARM_TARGET)

$(ARM_DEBIAN_DIR)/sectora_$(VERSION)_armhf.deb: src/main.rs src/daemon.rs src/lib.rs $(SRCS) $(CARGO_FILES) $(ASSETS)
	$(DOCKER_RUN) $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) sh -c "cargo install -f cargo-deb --root .cargo && CARGO_HOME=.cargo cargo deb --target=$(ARM_TARGET)"


.PHONY: clean clean-x64 clean-arm clean-exe clean-lib clean-deb clean-all

clean-x64:
	$(DOCKER_RUN) $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo clean

clean-arm:
	$(DOCKER_RUN) $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo clean

clean-exe:
	rm -f $(X64_RELEASE_DIR)/sectora
	rm -f $(ARM_RELEASE_DIR)/sectora

clean-daemon:
	rm -f $(X64_RELEASE_DIR)/sectorad
	rm -f $(ARM_RELEASE_DIR)/sectorad

clean-lib:
	rm -f $(X64_RELEASE_DIR)/libnss_sectora.so
	rm -f $(ARM_RELEASE_DIR)/libnss_sectora.so

clean-deb:
	rm -f $(X64_DEBIAN_DIR)/sectora_$(VERSION)_amd64.deb
	rm -f $(ARM_DEBIAN_DIR)/sectora_$(VERSION)_armhf.deb

clean:
	make clean-exe
	make clean-daemon
	make clean-lib
	make clean-deb

clean-all:
	$(DOCKER_RUN) $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo clean
	$(DOCKER_RUN) $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo clean
