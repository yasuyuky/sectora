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
	DEB_ARCH=amd64

	PKG_CONFIG_LIBDIR=/usr/lib/x86_64-linux-gnu/pkgconfig/

	# add openssl static link setting
	ENV_VALS=OPENSSL_STATIC=yes
	ENV_VALS+=OPENSSL_LIB_DIR=$(shell PKG_CONFIG_LIBDIR=$(PKG_CONFIG_LIBDIR) pkg-config --variable=libdir libssl)
	ENV_VALS+=OPENSSL_INCLUDE_DIR=$(shell PKG_CONFIG_LIBDIR=$(PKG_CONFIG_LIBDIR) pkg-config --variable=includedir libssl)
endif
ifeq ($(ARCH),arm)
	TARGET_TRIPLE=$(ARCH)-unknown-linux-gnueabi
	DEB_ARCH=armel

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


deb: $(TARGET_DIR)/sectora $(TARGET_DIR)/libnss_sectora.so
	rm -rf fakeroot/
	$(INSTALL) -d fakeroot/etc
	$(INSTALL) -d fakeroot/usr/$(sbindir)
	$(INSTALL) -d fakeroot/usr/$(libdir)
	$(INSTALL) -d fakeroot/debian/
	$(INSTALL_PROGRAM) $(TARGET_DIR)/sectora fakeroot/usr/$(sbindir)
	$(LIBTOOL) --mode=install $(INSTALL) $(TARGET_DIR)/libnss_sectora.so $(PWD)/fakeroot/usr/$(libdir)/
	cd $(PWD)/fakeroot/usr/$(libdir)/ && ln -sf libnss_sectora.so libnss_sectora.so.2
	$(INSTALL) -m660 -oroot -groot sectora.conf.template fakeroot/etc/sectora.conf.template
	$(INSTALL) -m755 debian/postinst fakeroot/debian/
	$(INSTALL) -m755 debian/postrm fakeroot/debian/
	$(INSTALL) -m755 debian/config fakeroot/debian/
	$(INSTALL) -m644 debian/control fakeroot/debian/
	$(INSTALL) -m644 debian/copyright fakeroot/debian/
	$(INSTALL) -m644 debian/templates fakeroot/debian/

	sed -i -e "s/^Architecture.*/Architecture : $(DEB_ARCH)/" fakeroot/debian/control
	fakeroot dpkg-deb --build fakeroot/ .

clean: FORCE
	cargo clean
	rm -rf fakeroot/

FORCE:
.PHONY: FORCE
