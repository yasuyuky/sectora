RUST_VER=$(shell cat rust-toolchain)

ARCH=x86_64
PROFILE=release

DESTDIR=/usr/local
bindir=/bin/
sbindir=/sbin/
libdir=/lib/

LIBTOOL=libtool
INSTALL=install
INSTALL_PROGRAM=$(INSTALL) -m555
INSTALL_DATA=$(INSTALL) -m444


ifeq ($(ARCH),x86_64)
	TARGET_TRIPLE=$(ARCH)-unknown-linux-gnu

	PKG_CONFIG_LIBDIR=/usr/lib/x86_64-linux-gnu/pkgconfig/

	# add openssl static link setting
	ENV_VALS=OPENSSL_STATIC=yes
	ENV_VALS+=OPENSSL_LIB_DIR=$(shell PKG_CONFIG_LIBDIR=$(PKG_CONFIG_LIBDIR) pkg-config --variable=libdir libssl)
	ENV_VALS+=OPENSSL_INCLUDE_DIR=$(shell PKG_CONFIG_LIBDIR=$(PKG_CONFIG_LIBDIR) pkg-config --variable=includedir libssl)
endif
ifeq ($(ARCH),arm)
	TARGET_TRIPLE=$(ARCH)-unknown-linux-gnueabi

	PKG_CONFIG_LIBDIR=/usr/lib/arm-linux-gnueabi/pkgconfig/

	ENV_VALS=OPENSSL_LIB_DIR=$(shell PKG_CONFIG_LIBDIR=$(PKG_CONFIG_LIBDIR) pkg-config --variable=libdir libssl)
	ENV_VALS+=OPENSSL_INCLUDE_DIR=$(shell PKG_CONFIG_LIBDIR=$(PKG_CONFIG_LIBDIR) pkg-config --variable=includedir libssl)
endif

TARGET_DIR=$(PWD)/target/$(TARGET_TRIPLE)/$(PROFILE)/


all: $(TARGET_DIR)/sectora $(TARGET_DIR)/libnss_sectora.so

$(TARGET_DIR)/sectora: FORCE
	$(ENV_VALS) cargo build --bin sectora --$(PROFILE) --target=$(TARGET_TRIPLE)

$(TARGET_DIR)/libnss_sectora.so: FORCE
	$(ENV_VALS) cargo build --lib --$(PROFILE) --target=$(TARGET_TRIPLE)

install: $(TARGET_DIR)/sectora $(TARGET_DIR)/libnss_sectora.so
	$(INSTALL_PROGRAM) $(TARGET_DIR)/sectora $(DESTDIR)/$(sbindir)
	$(INSTALL_PROGRAM) $(TARGET_DIR)/libnss_sectora.so $(DESTDIR)/$(libdir)
	$(LIBTOOL) --mode=install $(INSTALL) $(TARGET_DIR)/libnss_sectora.so $(DESTDIR)/$(libdir)/libnss_sectora.so
	cd $(DESTDIR)/$(libdir)/ && ln -sf libnss_sectora.so libnss_sectora.so.2



clean: FORCE
	cargo clean

FORCE:
.PHONY: FORCE
