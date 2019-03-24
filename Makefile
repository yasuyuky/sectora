RUST_VER=1.33.0
X64_TARGET=x86_64-unknown-linux-gnu
ARM_TARGET=arm-unknown-linux-gnueabihf
X64_TARGET_DIR=target/$(X64_TARGET)/release
ARM_TARGET_DIR=target/$(ARM_TARGET)/release
X64_BUILD_IMG=rust:${RUST_VER}-stretch
ARM_BUILD_IMG=yasuyuky/rust-arm:${RUST_VER}
COMMON_BUILD_OPT= -v ${PWD}:/source -w /source
OPENSSL_STATIC_OPT= -e OPENSSL_STATIC=yes -e OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu/ -e OPENSSL_INCLUDE_DIR=/usr/include
X64_BUILD_OPT= -v ${PWD}/.cargo-x64/registry:/usr/local/cargo/registry $(COMMON_BUILD_OPT) $(OPENSSL_STATIC_OPT)
ARM_BUILD_OPT= -v ${PWD}/.cargo-arm/registry:/usr/local/cargo/registry $(COMMON_BUILD_OPT)
DEPLOY_TEST_IMG=yasuyuky/ubuntu-ssh
ENTRIY_POINTS := src/main.rs src/lib.rs
SRCS := $(filter-out $(ENTRIY_POINTS),$(wildcard src/*.rs))
CARGO_FILES := Cargo.toml Cargo.lock

all: x64 arm

x64: x64-exe x64-lib

x64-exe: $(X64_TARGET_DIR)/sectora

x64-lib: $(X64_TARGET_DIR)/libnss_sectora.so

arm: arm-exe arm-lib

arm-exe: $(ARM_TARGET_DIR)/sectora

arm-lib: $(ARM_TARGET_DIR)/libnss_sectora.so

enter-build-image:
	docker run -it --rm $(X64_BUILD_OPT) $(X64_BUILD_IMG) bash

$(X64_TARGET_DIR)/sectora: src/main.rs $(SRCS) $(CARGO_FILES)
	docker run -it --rm $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo build --bin sectora --release --target=$(X64_TARGET)

$(X64_TARGET_DIR)/libnss_sectora.so: src/lib.rs $(SRCS) $(CARGO_FILES)
	docker run -it --rm $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo build --lib --release --target=$(X64_TARGET)

$(ARM_TARGET_DIR)/sectora: src/main.rs $(SRCS) $(CARGO_FILES)
	docker run -it --rm $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo build --bin sectora --release --target=$(ARM_TARGET)

$(ARM_TARGET_DIR)/libnss_sectora.so: src/lib.rs $(SRCS) $(CARGO_FILES)
	docker run -it --rm $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo build --lib --release --target=$(ARM_TARGET)


.PHONY: clean clean-x64 clean-arm clean-exe clean-lib clean-all

clean-x64:
	docker run -it --rm $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo clean

clean-arm:
	docker run -it --rm $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo clean

clean-exe:
	rm -f $(X64_TARGET_DIR)/sectora
	rm -f $(ARM_TARGET_DIR)/sectora

clean-lib:
	rm -f $(X64_TARGET_DIR)/libnss_sectora.so
	rm -f $(ARM_TARGET_DIR)/libnss_sectora.so

clean:
	make clean-exe
	make clean-lib

clean-all:
	docker run -it --rm $(X64_BUILD_OPT) $(X64_BUILD_IMG) cargo clean
	docker run -it --rm $(ARM_BUILD_OPT) $(ARM_BUILD_IMG) cargo clean
