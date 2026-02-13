// Projector: Output
//
// Subscribes to player, table, and hand domain events.
// Writes formatted game logs to a file.
package main

import (
	"fmt"
	"os"
	"strings"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
)

var logFile *os.File

func getLogFile() *os.File {
	if logFile == nil {
		path := os.Getenv("HAND_LOG_FILE")
		if path == "" {
			path = "hand_log.txt"
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

func getSequence(page *pb.EventPage) uint32 {
	if num, ok := page.Sequence.(*pb.EventPage_Num); ok {
		return num.Num
	}
	return 0
}

func handleEvents(events *pb.EventBook) (*pb.Projection, error) {
	if events == nil || events.Cover == nil {
		return &pb.Projection{}, nil
	}

	domain := events.Cover.Domain
	rootID := angzarr.RootIDText(events)

	var seq uint32

	for _, page := range events.Pages {
		if page.Event == nil {
			continue
		}
		seq = getSequence(page)

		typeURL := page.Event.TypeUrl
		typeName := typeURL[strings.LastIndex(typeURL, ".")+1:]

		msg := formatEvent(domain, rootID, typeName, page.Event.Value)
		writeLog(msg)
	}

	return &pb.Projection{
		Cover:     events.Cover,
		Projector: "output",
		Sequence:  seq,
	}, nil
}

func formatEvent(domain, rootID, typeName string, data []byte) string {
	switch typeName {
	case "PlayerRegistered":
		var e examples.PlayerRegistered
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("PLAYER %s registered: %s (%s)", rootID, e.DisplayName, e.Email)
		}
	case "FundsDeposited":
		var e examples.FundsDeposited
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("PLAYER %s deposited %d, balance: %d", rootID, e.Amount.Amount, e.NewBalance.Amount)
		}
	case "TableCreated":
		var e examples.TableCreated
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("TABLE %s created: %s (%s)", rootID, e.TableName, e.GameVariant.String())
		}
	case "PlayerJoined":
		var e examples.PlayerJoined
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("TABLE %s player %s joined with %d chips", rootID, angzarr.BytesToUUIDText(e.PlayerRoot), e.Stack)
		}
	case "HandStarted":
		var e examples.HandStarted
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("TABLE %s hand #%d started, %d players, dealer at position %d", rootID, e.HandNumber, len(e.ActivePlayers), e.DealerPosition)
		}
	case "CardsDealt":
		var e examples.CardsDealt
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("HAND %s cards dealt to %d players", rootID, len(e.PlayerCards))
		}
	case "BlindPosted":
		var e examples.BlindPosted
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("HAND %s player %s posted %s blind: %d", rootID, angzarr.BytesToUUIDText(e.PlayerRoot), e.BlindType, e.Amount)
		}
	case "ActionTaken":
		var e examples.ActionTaken
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("HAND %s player %s: %s %d", rootID, angzarr.BytesToUUIDText(e.PlayerRoot), e.Action.String(), e.Amount)
		}
	case "PotAwarded":
		var e examples.PotAwarded
		if err := proto.Unmarshal(data, &e); err == nil {
			winners := make([]string, len(e.Winners))
			for i, w := range e.Winners {
				winners[i] = fmt.Sprintf("%s wins %d", angzarr.BytesToUUIDText(w.PlayerRoot), w.Amount)
			}
			return fmt.Sprintf("HAND %s pot awarded: %s", rootID, strings.Join(winners, ", "))
		}
	case "HandComplete":
		var e examples.HandComplete
		if err := proto.Unmarshal(data, &e); err == nil {
			return fmt.Sprintf("HAND %s #%d complete", rootID, e.HandNumber)
		}
	}

	return fmt.Sprintf("%s.%s [%s]", domain, typeName, rootID)
}

func main() {
	// Clear log file at startup
	path := os.Getenv("HAND_LOG_FILE")
	if path == "" {
		path = "hand_log.txt"
	}
	os.Remove(path)

	handler := angzarr.NewProjectorHandler("output", "player", "table", "hand").
		WithHandle(handleEvents)

	angzarr.RunProjectorServer("output", "50290", handler)
}
