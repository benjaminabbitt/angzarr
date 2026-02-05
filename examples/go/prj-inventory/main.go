package main

import (
	"log"

	"angzarr"

	"projector-inventory/logic"
)

func main() {
	handler := angzarr.NewProjectorHandler(logic.ProjectorName, logic.SourceDomain).
		WithHandle(logic.Handle)

	cfg := angzarr.ServerConfig{Domain: logic.ProjectorName, DefaultPort: "50260"}
	if err := angzarr.RunProjectorServer(cfg, handler); err != nil {
		log.Fatal(err)
	}
}
