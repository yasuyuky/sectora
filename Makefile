TARGET=x86_64-unknown-linux-gnu
ARM_TARGET=arm-unknown-linux-gnueabihf
TARGET_DIR=target/$(TARGET)/release
ARM_TARGET_DIR=target/$(ARM_TARGET)/release
BUILD_IMG=yasuyuky/rust-ssl-static
ARM_BUILD_IMG=yasuyuky/rust-arm
BUILD_VOL_OPT= -v ${PWD}/.cargo/registry:/root/.cargo/registry -v ${PWD}:/source
ARM_BUILD_VOL_OPT= -v ${PWD}/.cargo/registry:/root/.cargo/registry -v ${PWD}:/source
DEPLOY_TEST_IMG=yasuyuky/ubuntu-ssh
SRCS := src/buffer.rs src/cstructs.rs src/ghclient.rs src/runfiles.rs src/statics.rs src/structs.rs

all: x64 arm

x64: $(TARGET_DIR)/ghteam-auth $(TARGET_DIR)/libnss_ghteam.so

arm: $(ARM_TARGET_DIR)/ghteam-auth $(ARM_TARGET_DIR)/libnss_ghteam.so

enter-build-image:
	docker run -it --rm -v ${PWD}:/source $(BUILD_IMG) bash

$(TARGET_DIR)/ghteam-auth: src/main.rs $(SRCS)
	docker run -it --rm $(BUILD_VOL_OPT) $(BUILD_IMG) cargo build --release --target=$(TARGET)

$(TARGET_DIR)/libnss_ghteam.so: src/lib.rs $(SRCS)
	docker run -it --rm $(BUILD_VOL_OPT) $(BUILD_IMG) cargo build --release --target=$(TARGET)

$(ARM_TARGET_DIR)/ghteam-auth: src/main.rs src/ghclient.rs src/structs.rs
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo build --release --target=$(ARM_TARGET)

$(ARM_TARGET_DIR)/libnss_ghteam.so: src/lib.rs src/ghclient.rs src/structs.rs
	docker run -it --rm $(ARM_BUILD_VOL_OPT) $(ARM_BUILD_IMG) cargo build --release --target=$(ARM_TARGET)


.PHONY: clean

clean:
	rm $(TARGET_DIR)/ghteam-auth
	rm $(TARGET_DIR)/libnss_ghteam.so
	rm $(ARM_TARGET_DIR)/ghteam-auth
	rm $(ARM_TARGET_DIR)/libnss_ghteam.so


