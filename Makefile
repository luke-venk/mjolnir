# `make up` rebuilds and spins containers up.
up:
	docker compose up --build

# `make down` brings containers back down.
down:
	docker compose down