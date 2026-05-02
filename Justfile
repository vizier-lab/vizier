install:
  @cargo install cargo-watch
  @echo -e "\\e[1;32minstalling core dependencies\\e[0m\n"
  cargo fetch
  @echo -e "\n\\e[1;32mDone\\e[0m"
  @echo -e "\n\\e[1;34minstalling webui dependencies\\e[0m\n"
  cd webui && npm i
  @echo -e "\n\\e[1;32mDone\\e[0m"

run:
  @cargo run -- run --config dev.vizier.yaml

run-a:
  @cargo run -- run -a --config dev.vizier.yaml

shutdown:
  @cargo run -- shutdown --config dev.vizier.yaml

dev:
  cargo watch -s "just run"

docker:
  @docker-compose down && docker-compose up -d

build:
  @cd webui && npm run build

release:
  @cargo build --release
