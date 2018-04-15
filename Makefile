RUST_VER=1.25.0
X64_TARGET=x86_64-unknown-linux-gnu
ARM_TARGET=arm-unknown-linux-gnueabihf
X64_TARGET_DIR=target/$(X64_TARGET)/release
ARM_TARGET_DIR=target/$(ARM_TARGET)/release
X64_BUILD_IMG=yasuyuky/rust-stable:${RUST_VER}
ARM_BUILD_IMG=yasuyuky/rust-arm:${RUST_VER}
X64_BUILD_VOL_OPT= -v ${PWD}/.cargo-x64/registry:/root/.cargo/registry -v ${PWD}:/source
ARM_BUILD_VOL_OPT= -v ${PWD}/.cargo-arm/registry:/root/.cargo/registry -v ${PWD}:/source
DEPLOY_TEST_IMG=yasuyuky/ubuntu-ssh
ENTRIY_POINTS := src/main.rs src/lib.rs
SRCS := $(filter-out $(ENTRIY_POINTS),$(wildcard src/*.rs))

all: x64 arm

x64: x64-exe x64-lib

x64-exe: $(X64_TARGET_DIR)/sectora

x64-lib: $(X64_TARGET_DIR)/libnss_sectora.so

arm: arm-exe arm-lib

arm-exe: $(ARM_TARGET_DIR)/sectora

arm-lib: $(ARM_TARGET_DIR)/libnss_sectora.so

enter-build-image:
	docker run -it --rm -v ${PWD}:/source $(X64_BUILD_IMG) bash

$(X64_TARGET_DIR)/sectora: src/main.rs $(SRCS)
	docker run -it --rm $(X64_BUILD_VOL_OPT) $(X64_BUILD_IMG) cargo build --bin sectora --release --target=$(X64_TARGET)

$(X64_TARGET_DIR)/libnss_sectora.so: src/lib.rs $(SRCS)
	docker run -it --rm $(X64_BUILD_VOL_OPT) $(X64_BUILD_IMG) cargo build --lib --release --target=$(X64_TARGET)

$(ARM_TARGET_DIR)/sectora: src/main.rs $(SRCS)
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo build --bin sectora --release --target=$(ARM_TARGET)

$(ARM_TARGET_DIR)/libnss_sectora.so: src/lib.rs $(SRCS)
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo build --lib --release --target=$(ARM_TARGET)


.PHONY: clean clean-x64 clean-arm clean-exe clean-lib

clean-x64:
	docker run -it --rm $(X64_BUILD_VOL_OPT) $(X64_BUILD_IMG) cargo clean

clean-arm:
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo clean

clean-exe:
	rm $(X64_TARGET_DIR)/sectora
	rm $(ARM_TARGET_DIR)/sectora

clean-lib:
	rm $(X64_TARGET_DIR)/libnss_sectora.so
	rm $(ARM_TARGET_DIR)/libnss_sectora.so

clean:
	docker run -it --rm $(X64_BUILD_VOL_OPT) $(X64_BUILD_IMG) cargo clean
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo clean
