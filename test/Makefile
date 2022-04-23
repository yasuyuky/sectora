VERSION=$(shell grep "^version" ../Cargo.toml | cut -f 2 -d '"')
dist:=ubuntu
ver:=bionic

.PHONY: test-ansible test-deb up down reset

test-ansible:
	make up
	make setup-ansible
	make exec-login
	make down

test-deb:
	make up
	make setup-deb
	make exec-login
	make down

setup-deb:
	@echo '::group::Installing deb packages'
	make -j 2 setup-host-deb setup-client-key
	@echo '::endgroup'

setup-ansible:
	@echo '::group::Installing ansible packages'
	make -j 2 setup-host-root setup-client-ansible
	docker-compose exec client ansible-playbook -i hosts localtest.yml
	sleep 1
	@echo '::endgroup'

setup-host-root:
	docker cp ./keys/root/id_rsa.pub `docker-compose ps -q host`:/root/.ssh/authorized_keys
	docker-compose exec host chown -R root:root /root/.ssh/
	docker-compose exec host chmod -R 600 /root/.ssh/

setup-client-key:
	docker cp ./keys `docker-compose ps -q client`:/work/keys

setup-host-deb:
	docker cp ../target/x86_64-unknown-linux-gnu/debian/sectora_$(VERSION)_amd64.deb `docker-compose ps -q host`:/tmp/
	docker-compose exec -T host sh -c "DEBIAN_FRONTEND=noninteractive apt-get install -y /tmp/sectora_$(VERSION)_amd64.deb"
	docker cp ./testconf.toml `docker-compose ps -q host`:/etc/sectora.conf
	time docker-compose exec -T host systemctl start sectora
	time docker-compose exec -T host systemctl restart ssh

setup-client-ansible:
	make setup-client-key
	docker cp ./client/hosts `docker-compose ps -q client`:/work/
	docker cp ./client/localtest.yml `docker-compose ps -q client`:/work/
	docker cp ../ansible/roles `docker-compose ps -q client`:/work/roles
	docker cp ../ansible/templates `docker-compose ps -q client`:/work/templates
	docker cp ../assets/sectora.service `docker-compose ps -q client`:/work/
	docker cp ../assets/sectora.sh `docker-compose ps -q client`:/work/
	docker cp ../target/x86_64-unknown-linux-gnu/release `docker-compose ps -q client`:/work/release

up:
	@echo '::group::Starting containers'
	@echo '$(shell tput setaf 6)$(dist) $(shell tput setaf 3)$(ver)$(shell tput sgr 0)'
	dist=$(dist) ver=$(ver) docker-compose up -d --build
	@echo '::endgroup'

exec-login:
	@echo '$(shell tput setaf 6)LOGIN TEST START$(shell tput sgr 0)'
	@echo '$(shell tput setab 7)$(shell tput setaf 0) SSH $(shell tput sgr 0)'
	@docker-compose exec -T client ssh hunter@host -i keys/user/id_rsa \
		echo '"$(shell tput setaf 2)SUCCESS$(shell tput sgr 0)"'
	@docker-compose exec -T client ssh hunter@host -i keys/user/id_rsa \
		echo '::notice title=$(dist)/$(ver)::success'
	@echo '$(shell tput setab 7)$(shell tput setaf 0) ID $(shell tput sgr 0)'
	@docker-compose exec -T client ssh hunter@host -i keys/user/id_rsa id
	@echo '$(shell tput setab 7)$(shell tput setaf 0) VERSION $(shell tput sgr 0)'
	@docker-compose exec -T client ssh hunter@host -i keys/user/id_rsa /usr/sbin/sectora version
	@echo '$(shell tput setaf 6)LOGIN TEST END$(shell tput sgr 0)'

down:
	@echo '::group::Stopping containers'
	docker-compose down
	@echo '::endgroup'

reset:
	cd .. && make x64 LOG_LEVEL=DEBUG
	make down
	make up
	make setup

restart:
	docker-compose restart sshd
