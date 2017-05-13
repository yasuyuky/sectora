TARGET=x86_64-unknown-linux-gnu
TARGET_DIR=target/$(TARGET)/release
BUILD_IMG=yasuyuky/rust-ssl-static
BUILD_VOL_OPT= -v ${PWD}/.cargo/registry:/root/.cargo/registry -v ${PWD}:/source

all: $(TARGET_DIR)/ghteam-auth $(TARGET_DIR)/libnss_ghteam.so

enter-build-image:
	docker run -it --rm -v ${PWD}:/source $(BUILD_IMG) bash

$(TARGET_DIR)/ghteam-auth: src
	docker run -it --rm $(BUILD_VOL_OPT) $(BUILD_IMG) cargo build --release --target=$(TARGET)

$(TARGET_DIR)/libnss_ghteam.so: src
	docker run -it --rm $(BUILD_VOL_OPT) $(BUILD_IMG) cargo build --release --target=$(TARGET)

.PHONY: clean

clean:
	rm $(TARGET_DIR)/ghteam-auth
	rm $(TARGET_DIR)/libnss_ghteam.so

