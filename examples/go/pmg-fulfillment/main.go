package main

import (
	"log"

	"angzarr"

	"process-manager-fulfillment/logic"
)

func main() {
	handler := angzarr.NewProcessManagerHandler(logic.PMName).
		ListenTo("order").
		ListenTo("inventory").
		ListenTo("fulfillment").
		WithHandle(logic.Handle)

	cfg := angzarr.ServerConfig{Domain: logic.PMName, DefaultPort: "50220"}
	if err := angzarr.RunProcessManagerServer(cfg, handler); err != nil {
		log.Fatal(err)
	}
}
