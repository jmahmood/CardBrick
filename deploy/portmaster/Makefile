# CardBrick-Py experimental PortMaster packaging.

DECKS ?=

.PHONY: runtime package validate deploy-ssh clean distclean

runtime:
	./scripts/build_runtime.sh

package:
	CARDBRICK_DECKS="$(DECKS)" ./scripts/build_package.sh
	./scripts/validate_package.sh

validate:
	./scripts/validate_package.sh

deploy-ssh:
	@test -n "$(HOST)" || (echo "Usage: make -C deploy/portmaster deploy-ssh HOST=user@device [SMOKE=1]" >&2; exit 2)
	./scripts/deploy_ssh.sh --host "$(HOST)" $(if $(PARALLEL),--parallel,) $(if $(REPLACE),--replace,) $(if $(SMOKE),--smoke,)

clean:
	rm -rf dist

distclean: clean
	rm -rf runtime-build/build
