RUST_VER=$(shell cat rust-toolchain)
VERSION=$(shell grep "^version" Cargo.toml | cut -f 2 -d '"')
TARGET:=amd64
LOG_LEVEL:=OFF
BUILD_IMG=ghcr.io/yasuyuky/rust-ubuntu:${RUST_VER}
PLATFORM_OPT=--platform=linux/$(TARGET)
ifeq ($(TARGET),amd64)
RSTARGET=x86_64-unknown-linux-gnu
else ifeq ($(TARGET),arm64)
RSTARGET=aarch64-unknown-linux-gnu
else ifeq ($(TARGET),armhf)
RSTARGET=armv7-unknown-linux-gnueabihf
LIBTARGET=arm-linux-gnueabihf
endif
LIBTARGET?=$(subst unknown-,,$(RSTARGET))
RELEASE_DIR=target/$(RSTARGET)/release
DEBIAN_DIR=target/$(RSTARGET)/debian
COMMON_BUILD_OPT= -v ${PWD}:/source -w /source
# BUILD_VOL= -v ${PWD}/.cargo-$(TARGET)/registry:/usr/local/cargo/registry -v ${PWD}/.cargo-$(TARGET)/bin:/source/.cargo/bin
# BUILD_VOL= -v ${PWD}/.cargo-$(TARGET):/source/.cargo
BUILD_VOL= -v ${PWD}/.sccache-$(TARGET):/source/.sccache
OPENSSL_STATIC_OPT= -e OPENSSL_STATIC=yes -e OPENSSL_LIB_DIR=/usr/lib/$(LIBTARGET)/ -e OPENSSL_INCLUDE_DIR=/usr/include -e LOG_LEVEL=$(LOG_LEVEL)
BUILD_OPT= $(BUILD_VOL) $(COMMON_BUILD_OPT) $(OPENSSL_STATIC_OPT)
DEPLOY_TEST_IMG=yasuyuky/ubuntu-ssh
ENTRIY_POINTS := src/main.rs src/daemon.rs src/lib.rs
SRCS := $(filter-out $(ENTRIY_POINTS),$(wildcard src/*.rs))
ASSETS := $(wildcard assets/*) $(wildcard assets/*/*)
CARGO_FILES := Cargo.toml Cargo.lock rust-toolchain
DOCKER_RUN=docker run --rm $(PLATFORM_OPT)

all:

deb: $(DEBIAN_DIR)/sectora_$(VERSION)_$(TARGET).deb

exe: $(RELEASE_DIR)/sectora

daemon: $(RELEASE_DIR)/sectorad

lib: $(RELEASE_DIR)/libnss_sectora.so

amd64:
	make TARGET=amd64 exe daemon lib deb

armhf:
	make TARGET=armhf exe daemon lib deb

arm64:
	make TARGET=arm64 exe daemon lib deb

enter-build-image:
	$(DOCKER_RUN) -it $(BUILD_OPT) $(BUILD_IMG) bash

$(RELEASE_DIR)/sectora: src/main.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(BUILD_OPT) $(BUILD_IMG) cargo build --bin sectora --release --target=$(RSTARGET)

$(RELEASE_DIR)/sectorad: src/daemon.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(BUILD_OPT) $(BUILD_IMG) cargo build --bin sectorad --release --target=$(RSTARGET)

$(RELEASE_DIR)/libnss_sectora.so: src/lib.rs $(SRCS) $(CARGO_FILES)
	$(DOCKER_RUN) $(BUILD_OPT) $(BUILD_IMG) cargo build --lib --release --target=$(RSTARGET)

$(DEBIAN_DIR)/sectora_$(VERSION)_$(TARGET).deb: src/main.rs src/daemon.rs src/lib.rs $(SRCS) $(CARGO_FILES) $(ASSETS)
	$(DOCKER_RUN) $(BUILD_OPT) $(BUILD_IMG) sh -c "cargo install cargo-deb --root .cargo && CARGO_HOME=.cargo cargo deb --target=$(RSTARGET)"

.PHONY: clean clean-cargo clean-exe clean-lib clean-deb

clean-cargo:
	$(DOCKER_RUN) $(BUILD_OPT) $(BUILD_IMG) cargo clean

clean-exe:
	rm -f target/*/release/sectora

clean-daemon:
	rm -f target/*/release/sectorad

clean-lib:
	rm -f target/*/release/libnss_sectora.so

clean-deb:
	rm -f target/*/debian/sectora_$(VERSION)_*.deb

clean:
	make clean-exe
	make clean-daemon
	make clean-lib
	make clean-deb
