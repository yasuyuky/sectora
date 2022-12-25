VERSION=$(shell grep "^version" ../Cargo.toml | cut -f 2 -d '"')
dist:=ubuntu
ver:=bionic
user:=hunter

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

setup-client-key:
	docker cp ./keys client:/work/keys

setup-host-deb:
	docker cp ../target/x86_64-unknown-linux-gnu/debian/sectora_$(VERSION)_amd64.deb host:/tmp/
	docker exec host sh -c "DEBIAN_FRONTEND=noninteractive apt-get install -y /tmp/sectora_$(VERSION)_amd64.deb"
	docker cp ./testconf.toml host:/etc/sectora.conf
	time docker exec host systemctl start sectora
	time docker exec host systemctl restart ssh

setup-client-ansible:
	make setup-client-key
	docker cp ./client/hosts client:/work/
	docker cp ./client/localtest.yml client:/work/
	docker cp ../ansible/roles client:/work/roles
	docker cp ../ansible/templates client:/work/templates
	docker cp ../assets/sectora.service client:/work/
	docker cp ../assets/sectora.sh client:/work/
	docker cp ../target/x86_64-unknown-linux-gnu/release client:/work/release

up:
	@echo '::group::Starting containers'
	@echo '$(shell tput setaf 6)$(dist) $(shell tput setaf 3)$(ver)$(shell tput sgr 0)'
	docker network create -d bridge testnw
	docker build -t json-server ./json-server
	docker run -d --network testnw --name json-server -h json-server json-server json-server --watch db.json --host 0.0.0.0 --routes routes.json
	docker run -d --network testnw --name host -h host --privileged --cgroupns=host -v /sys/fs/cgroup:/sys/fs/cgroup:rw yasuyuky/ssh-test:$(dist).$(ver)
	docker build -t client client
	docker run -d --network testnw --name client -h client -w /work client sh -c 'while true; do sleep 1; done'
	@echo '::endgroup::'

exec-login:
	@echo '$(shell tput setaf 6)LOGIN TEST START$(shell tput sgr 0)'
	@echo '$(shell tput setab 7)$(shell tput setaf 0) SSH $(shell tput sgr 0)'
	@docker exec client ssh $(user)@host -i keys/user/id_rsa \
		echo '"$(shell tput setaf 2)SUCCESS$(shell tput sgr 0)"'
	@docker exec client ssh $(user)@host -i keys/user/id_rsa \
		echo '::notice title=$(dist)/$(ver)::success'
	@echo '$(shell tput setab 7)$(shell tput setaf 0) ID $(shell tput sgr 0)'
	@docker exec client ssh $(user)@host -i keys/user/id_rsa id
	@echo '$(shell tput setab 7)$(shell tput setaf 0) VERSION $(shell tput sgr 0)'
	@docker exec client ssh $(user)@host -i keys/user/id_rsa /usr/sbin/sectora version
	@echo '$(shell tput setaf 6)LOGIN TEST END$(shell tput sgr 0)'

down:
	@echo '::group::Stopping containers'
	docker rm -f json-server
	docker rm -f host
	docker rm -f client
	docker network rm testnw
	@echo '::endgroup::'

reset:
	cd .. && make x64 LOG_LEVEL=DEBUG
	make down
	make up
	make setup

restart:
	docker restart host
