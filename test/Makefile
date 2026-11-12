VERSION=$(shell grep "^version" ../Cargo.toml | cut -f 2 -d '"')
dist:=ubuntu
ver?=jammy
user:=hunter
keyfile:=id_rsa
target ?= x86_64-unknown-linux-gnu
DEB:=$(firstword $(wildcard ../target/$(target)/debian/sectora_$(VERSION)*.deb))
DEB_BASENAME:=$(notdir $(DEB))
DEB_DST:=/root/$(DEB_BASENAME)

ifeq ($(strip $(DEB)),)
$(error deb package not found: ../target/$(target)/debian/sectora_$(VERSION)*.deb (run make TARGET=$(target) deb first))
endif

.PHONY: test-ansible test-deb up down reset

test-ansible-stub:
	make up
	make setup-ansible
	make exec-login
	make exec-password-denied
	make exec-account-denied
	make exec-ansible-upgrade
	make down

test-deb-stub:
	make up
	make create-test-conf-stub
	make setup-deb
	make exec-socket-safety
	make exec-login
	make exec-password-denied
	make exec-account-denied
	make exec-password-denied-upgrade
	make down

test-deb:
	make up
	make create-test-conf-env
	make setup-deb
	make setup-client-key-env
	make user=${TEST_USER} exec-login
	make user=${TEST_USER} exec-password-denied
	make user=${TEST_USER} exec-account-denied
	make down

setup-deb:
	@echo '::group::Installing deb packages'
	make -j 2 setup-host-deb setup-client-key
	@echo '::endgroup::'

setup-ansible:
	@echo '::group::Installing ansible packages'
	make -j 2 setup-host-root setup-client-ansible
	docker exec client ansible-playbook -i hosts localtest.yml
	sleep 1
	@echo '::endgroup::'

setup-host-root:
	docker cp ./keys/root/id_rsa.pub host:/root/.ssh/authorized_keys
	docker exec host chown -R root:root /root/.ssh/
	docker exec host chmod -R 600 /root/.ssh/

enter-host:
	docker exec -it host bash

setup-client-key:
	docker cp ./keys client:/work/keys

setup-client-key-env:
	@echo "$${TEST_PRIVATE_KEY}" > $(keyfile)
	chmod 600 $(keyfile)
	docker exec client rm -rf /work/keys
	docker exec client mkdir -p /work/keys/user/
	docker cp $(keyfile) client:/work/keys/user/$(keyfile)
	rm $(keyfile)

create-test-conf-stub:
	@echo 'token = "TESTTOKEN"'                   > testconf.toml
	@echo 'org = "soundtribe"'                   >> testconf.toml
	@echo 'endpoint = "http://json-server:3000"' >> testconf.toml
	@echo                                        >> testconf.toml
	@echo '[[team]]'                             >> testconf.toml
	@echo 'name = "sector9"'                     >> testconf.toml
	@echo 'gid = 2019'                           >> testconf.toml

create-test-conf-env:
	@echo "token = \"${TEST_GITHUB_TOKEN}\""  > testconf.toml
	@echo "org = \"${TEST_GITHUB_ORG}\""     >> testconf.toml
	@echo                                    >> testconf.toml
	@echo '[[team]]'                         >> testconf.toml
	@echo "name = \"${TEST_GITHUB_TEAM}\""   >> testconf.toml
	@echo "gid = 2022"                       >> testconf.toml

setup-host-deb:
	docker cp $(DEB) host:$(DEB_DST)
	docker exec host sh -c 'set -e; test -f $(DEB_DST); DEBIAN_FRONTEND=noninteractive dpkg -i $(DEB_DST) || { DEBIAN_FRONTEND=noninteractive apt-get -f install -y; DEBIAN_FRONTEND=noninteractive dpkg -i $(DEB_DST); }'
	docker cp ./testconf.toml host:/etc/sectora.conf
	time docker exec host systemctl start sectora
	sleep 3
	docker exec host systemctl status sectora
	@echo '::group::Diagnostic checks'
	-docker exec host cat /etc/nsswitch.conf
	-docker exec host getent passwd hunter
	-docker exec host cat /etc/pam.d/sshd
	-docker exec host cat /etc/pam.d/common-auth
	-docker exec host sh -c "ls -la /usr/lib/libnss_sectora.so*"
	-docker exec host cat /etc/pam.d/sshd
	-docker exec host cat /etc/pam.d/common-account
	-docker exec host ls -la /usr/sbin/unix_chkpwd
	@echo '::endgroup::'
	time docker exec host systemctl restart ssh
	sleep 2

setup-client-ansible:
	make setup-client-key
	docker cp ./client/hosts client:/work/
	docker cp ./client/localtest.yml client:/work/
	docker cp ../ansible/roles client:/work/roles
	docker cp ../ansible/templates client:/work/templates
	docker cp ../assets/sectora.service client:/work/
	docker cp ../assets/sectora.sh client:/work/
	docker exec client mkdir -p /work/release
	docker cp ../target/$(target)/release/sectora client:/work/release/
	docker cp ../target/$(target)/release/sectorad client:/work/release/
	docker cp ../target/$(target)/release/libnss_sectora.so client:/work/release/

up:
	@echo '::group::Starting containers'
	@echo '$(shell tput setaf 6)$(dist) $(shell tput setaf 3)$(ver)$(shell tput sgr 0)'
	docker network create -d bridge testnw
	docker build -t json-server ./json-server
	docker run -d --network testnw --name json-server json-server json-server --watch db.json --host 0.0.0.0 --routes routes.json
	docker run -d --network testnw --name host --privileged --cgroupns=host -v /sys/fs/cgroup:/sys/fs/cgroup:rw yasuyuky/ssh-test:$(dist).$(ver)
	docker build -t client client
	docker run -d --network testnw --name client -w /work client sh -c 'while true; do sleep 1; done'
	@echo '::endgroup::'

exec-login:
	@echo '$(shell tput setaf 6)LOGIN TEST START$(shell tput sgr 0)'
	@echo '$(shell tput setab 7)$(shell tput setaf 0) SSH $(shell tput sgr 0)'
	@docker exec client ssh -vvv $(user)@host -i keys/user/$(keyfile) \
		echo '"$(shell tput setaf 2)SUCCESS$(shell tput sgr 0)"' || \
		(echo ':: group::SSH Server Logs' && \
		docker exec host journalctl -u ssh --no-pager -n 100 && \
		docker exec host journalctl -u sectora --no-pager -n 100 && \
		echo '::endgroup:: ' && exit 1)
	@docker exec client ssh $(user)@host -i keys/user/$(keyfile) \
		echo '::notice title=$(dist)/$(ver)::success'
	@echo '$(shell tput setab 7)$(shell tput setaf 0) ID $(shell tput sgr 0)'
	@docker exec client ssh $(user)@host -i keys/user/$(keyfile) id
	@echo '$(shell tput setab 7)$(shell tput setaf 0) VERSION $(shell tput sgr 0)'
	@docker exec client ssh $(user)@host -i keys/user/$(keyfile) /usr/sbin/sectora version
	@echo '$(shell tput setaf 6)LOGIN TEST END$(shell tput sgr 0)'

exec-password-denied:
	@docker exec host sh -c "sshd -T | grep -qx 'usepam yes'"
	@docker exec host sh -c "sshd -T | grep -qx 'passwordauthentication yes'"
	@docker exec host sh -c "sshd -T | grep -qx 'authenticationmethods any'"
	@docker exec client ssh -o BatchMode=yes -o IdentitiesOnly=yes -i keys/user/$(keyfile) $(user)@host true
	@output=$$(docker exec client sshpass -p deliberately-wrong-password ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no -o KbdInteractiveAuthentication=no -o NumberOfPasswordPrompts=1 -o ConnectTimeout=10 $(user)@host true 2>&1); status=$$?; test "$$status" -ne 0 && printf '%s\n' "$$output" | grep -q 'Permission denied'

exec-socket-safety:
	docker exec -i host bash < socket-startup.sh

exec-ansible-upgrade:
	@docker exec host sed -i '1i account sufficient pam_exec.so quiet /usr/sbin/sectora pam\n\taccount  sufficient\tpam_exec.so  quiet /usr/sbin/sectora pam  # legacy rule\nsession required pam_mkhomedir.so skel: /etc/skel/ umask: 0022' /etc/pam.d/sshd
	docker exec client ansible-playbook -i hosts localtest.yml
	@$(MAKE) exec-password-denied
	@$(MAKE) exec-account-denied

exec-password-denied-upgrade:
	@docker exec host sed -i '/^### DO NOT REMOVE! SECTORA SETTINGS START ###$$/a auth sufficient pam_exec.so quiet /usr/sbin/sectora pam\n\tauth  sufficient\tpam_exec.so  quiet /usr/sbin/sectora pam  # legacy rule' /etc/pam.d/sshd
	@docker exec host sed -i '/^### DO NOT REMOVE! SECTORA SETTINGS START ###$$/a account sufficient pam_permit.so\n\taccount  sufficient\tpam_permit.so  # legacy rule' /etc/pam.d/sshd
	@docker exec -e DEBIAN_FRONTEND=noninteractive host /var/lib/dpkg/info/sectora.postinst configure
	@$(MAKE) exec-password-denied
	@$(MAKE) exec-account-denied

exec-account-denied:
	@docker exec host sh -c "grep -qx '@include common-account' /etc/pam.d/sshd"
	@docker exec client ssh -o BatchMode=yes -o IdentitiesOnly=yes -i keys/user/$(keyfile) $(user)@host true
	@docker exec host sed -i '/^@include common-account$$/i account requisite pam_deny.so' /etc/pam.d/sshd
	@status=0; docker exec client ssh -o BatchMode=yes -o IdentitiesOnly=yes -o ConnectTimeout=10 -i keys/user/$(keyfile) $(user)@host true || status=$$?; docker exec host sed -i '/^account requisite pam_deny.so$$/d' /etc/pam.d/sshd && test $$status -ne 0
	@docker exec client ssh -o BatchMode=yes -o IdentitiesOnly=yes -i keys/user/$(keyfile) $(user)@host true

down:
	@echo '::group::Stopping containers'
	docker rm -f json-server
	docker rm -f host
	docker rm -f client
	docker network rm testnw
	@echo '::endgroup::'

reset:
	cd .. && make TARGET=$(target) deb LOG_LEVEL=DEBUG
	make down
	make up
	make setup

restart:
	docker restart host
