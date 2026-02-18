// Projector: Output (OO Pattern)
//
// Subscribes to player, table, and hand domain events.
// Writes formatted game logs to a file.
//
// This is the OO-style implementation using ProjectorBase with method-based
// handlers and fluent registration. Contrasts with prj-output/ which
// uses the functional ProjectorHandler pattern.
package main

import (
	"fmt"
	"os"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
)

var logFile *os.File

func getLogFile() *os.File {
	if logFile == nil {
		path := os.Getenv("HAND_LOG_FILE")
		if path == "" {
			path = "hand_log_oo.txt"
		}
		var err error
		logFile, err = os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to open log file: %v\n", err)
		}
	}
	return logFile
}

func writeLog(msg string) {
	f := getLogFile()
	if f != nil {
		timestamp := time.Now().Format("2006-01-02T15:04:05.000")
		f.WriteString(fmt.Sprintf("[%s] %s\n", timestamp, msg))
	}
}

// docs:start:projector_oo
// OutputProjector writes game events to a log file.
type OutputProjector struct {
	angzarr.ProjectorBase
}

// NewOutputProjector creates a new OutputProjector with registered handlers.
func NewOutputProjector() *OutputProjector {
	p := &OutputProjector{}
	p.Init("output", []string{"player", "table", "hand"})

	// Register projection handlers
	p.Projects("PlayerRegistered", p.projectRegistered)
	p.Projects("FundsDeposited", p.projectDeposited)
	p.Projects("TableCreated", p.projectTableCreated)
	p.Projects("PlayerJoined", p.projectPlayerJoined)
	p.Projects("HandStarted", p.projectHandStarted)
	p.Projects("CardsDealt", p.projectCardsDealt)
	p.Projects("BlindPosted", p.projectBlindPosted)
	p.Projects("ActionTaken", p.projectActionTaken)
	p.Projects("PotAwarded", p.projectPotAwarded)
	p.Projects("HandComplete", p.projectHandComplete)

	return p
}

func (p *OutputProjector) projectRegistered(event *examples.PlayerRegistered) *pb.Projection {
	writeLog(fmt.Sprintf("PLAYER registered: %s (%s)", event.DisplayName, event.Email))
	return nil
}

func (p *OutputProjector) projectDeposited(event *examples.FundsDeposited) *pb.Projection {
	amount := int64(0)
	newBalance := int64(0)
	if event.Amount != nil {
		amount = event.Amount.Amount
	}
	if event.NewBalance != nil {
		newBalance = event.NewBalance.Amount
	}
	writeLog(fmt.Sprintf("PLAYER deposited %d, balance: %d", amount, newBalance))
	return nil
}

func (p *OutputProjector) projectTableCreated(event *examples.TableCreated) *pb.Projection {
	writeLog(fmt.Sprintf("TABLE created: %s (%s)", event.TableName, event.GameVariant.String()))
	return nil
}

func (p *OutputProjector) projectPlayerJoined(event *examples.PlayerJoined) *pb.Projection {
	playerID := angzarr.BytesToUUIDText(event.PlayerRoot)
	writeLog(fmt.Sprintf("TABLE player %s joined with %d chips", playerID, event.Stack))
	return nil
}

func (p *OutputProjector) projectHandStarted(event *examples.HandStarted) *pb.Projection {
	writeLog(fmt.Sprintf("TABLE hand #%d started, %d players, dealer at position %d",
		event.HandNumber, len(event.ActivePlayers), event.DealerPosition))
	return nil
}

func (p *OutputProjector) projectCardsDealt(event *examples.CardsDealt) *pb.Projection {
	writeLog(fmt.Sprintf("HAND cards dealt to %d players", len(event.PlayerCards)))
	return nil
}

func (p *OutputProjector) projectBlindPosted(event *examples.BlindPosted) *pb.Projection {
	playerID := angzarr.BytesToUUIDText(event.PlayerRoot)
	writeLog(fmt.Sprintf("HAND player %s posted %s blind: %d", playerID, event.BlindType, event.Amount))
	return nil
}

func (p *OutputProjector) projectActionTaken(event *examples.ActionTaken) *pb.Projection {
	playerID := angzarr.BytesToUUIDText(event.PlayerRoot)
	writeLog(fmt.Sprintf("HAND player %s: %s %d", playerID, event.Action.String(), event.Amount))
	return nil
}

func (p *OutputProjector) projectPotAwarded(event *examples.PotAwarded) *pb.Projection {
	winners := make([]string, len(event.Winners))
	for i, w := range event.Winners {
		winners[i] = fmt.Sprintf("%s wins %d", angzarr.BytesToUUIDText(w.PlayerRoot), w.Amount)
	}
	writeLog(fmt.Sprintf("HAND pot awarded: %v", winners))
	return nil
}

func (p *OutputProjector) projectHandComplete(event *examples.HandComplete) *pb.Projection {
	writeLog(fmt.Sprintf("HAND #%d complete", event.HandNumber))
	return nil
}

// docs:end:projector_oo

func main() {
	// Clear log file at startup
	path := os.Getenv("HAND_LOG_FILE")
	if path == "" {
		path = "hand_log_oo.txt"
	}
	os.Remove(path)

	projector := NewOutputProjector()
	angzarr.RunOOProjectorServer("output", "50291", &projector.ProjectorBase)
}
