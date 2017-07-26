X64_TARGET=x86_64-unknown-linux-gnu
ARM_TARGET=arm-unknown-linux-gnueabihf
X64_TARGET_DIR=target/$(X64_TARGET)/release
ARM_TARGET_DIR=target/$(ARM_TARGET)/release
X64_BUILD_IMG=yasuyuky/rust-ssl-static
ARM_BUILD_IMG=yasuyuky/rust-arm
X64_BUILD_VOL_OPT= -v ${PWD}/.cargo-x64/registry:/root/.cargo/registry -v ${PWD}:/source
ARM_BUILD_VOL_OPT= -v ${PWD}/.cargo-arm/registry:/root/.cargo/registry -v ${PWD}:/source
DEPLOY_TEST_IMG=yasuyuky/ubuntu-ssh
SRCS := src/buffer.rs src/cstructs.rs src/ghclient.rs src/runfiles.rs src/statics.rs src/structs.rs

all: x64 arm

x64: $(X64_TARGET_DIR)/ghteam-auth $(X64_TARGET_DIR)/libnss_ghteam.so

arm: $(ARM_TARGET_DIR)/ghteam-auth $(ARM_TARGET_DIR)/libnss_ghteam.so

enter-build-image:
	docker run -it --rm -v ${PWD}:/source $(X64_BUILD_IMG) bash

$(X64_TARGET_DIR)/ghteam-auth: src/main.rs $(SRCS)
	docker run -it --rm $(X64_BUILD_VOL_OPT) $(X64_BUILD_IMG) cargo build --bin ghteam-auth --release --target=$(X64_TARGET)

$(X64_TARGET_DIR)/libnss_ghteam.so: src/lib.rs $(SRCS)
	docker run -it --rm $(X64_BUILD_VOL_OPT) $(X64_BUILD_IMG) cargo build --lib --release --target=$(X64_TARGET)

$(ARM_TARGET_DIR)/ghteam-auth: src/main.rs src/ghclient.rs src/structs.rs
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo build --bin ghteam-auth --release --target=$(ARM_TARGET)

$(ARM_TARGET_DIR)/libnss_ghteam.so: src/lib.rs src/ghclient.rs src/structs.rs
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo build --lib --release --target=$(ARM_TARGET)


.PHONY: clean

clean:
	rm $(X64_TARGET_DIR)/ghteam-auth
	rm $(X64_TARGET_DIR)/libnss_ghteam.so
	rm $(ARM_TARGET_DIR)/ghteam-auth
	rm $(ARM_TARGET_DIR)/libnss_ghteam.so


