# CardBrick — Knulli / RG35XX SP packaging (pygame-ce squashfs runtime).
# Run from deploy/knulli/. See PACKAGING.md for the full walkthrough.
#
#   make runtime          build the ARM64 squashfs runtime (Docker, slow, once)
#   make package          stage dist/ + zip (needs the runtime, or
#                         RUNTIME_SQUASHFS=/path/to/file.squashfs)
#   make package-fast     app-only package, no runtime (quick iteration)
#   make validate         check the staged package before deploying
#   make deploy-sd        copy to a mounted SD card   [SD=/media/me/SHARE]
#   make deploy-ssh       push over SSH               [HOST=root@192.168.1.42]
#   make smoke-ssh        deploy over SSH + run the on-device smoke test
#   make clean            remove dist/ (keeps the built runtime)
#   make distclean        remove dist/ AND the built runtime

SD    ?=
HOST  ?=

HOSTFLAG = $(if $(HOST),--host $(HOST),)

.PHONY: runtime package package-fast validate deploy-sd deploy-ssh smoke-ssh clean distclean

runtime:
	scripts/build_runtime.sh

package:
	scripts/build_package.sh
	scripts/validate_package.sh

package-fast:
	scripts/build_package.sh --no-runtime
	scripts/validate_package.sh

validate:
	scripts/validate_package.sh

deploy-sd:
	scripts/deploy_sdcard.sh $(SD)

deploy-ssh:
	scripts/deploy_ssh.sh $(HOSTFLAG)

smoke-ssh:
	scripts/deploy_ssh.sh $(HOSTFLAG) --smoke

clean:
	rm -rf dist

distclean: clean
	rm -rf runtime-build/build
