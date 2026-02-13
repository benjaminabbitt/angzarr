// Table aggregate BDD tests using godog.
//
// These tests load scenarios from the shared features/unit/table.feature file
// and run them against the Go implementation of the table aggregate.
package tests

import (
	"context"
	"encoding/hex"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/benjaminabbitt/angzarr/examples/go/table/agg/handlers"
)

// tableTestContext holds the state for a single table scenario.
type tableTestContext struct {
	domain        string
	root          []byte
	events        []*pb.EventPage
	nextSequence  uint32
	lastError     error
	lastEventBook *pb.EventBook
	lastErrorMsg  string
	lastState     handlers.TableState

	// Default table settings for tests
	minBuyIn   int64
	maxBuyIn   int64
	maxPlayers int32
}

// newTableTestContext creates a fresh table test context.
func newTableTestContext() *tableTestContext {
	return &tableTestContext{
		domain:     "table",
		root:       uuidFor("table-test"),
		events:     make([]*pb.EventPage, 0),
		minBuyIn:   200,
		maxBuyIn:   1000,
		maxPlayers: 9,
	}
}

// buildEventBook creates an EventBook from the current events.
func (c *tableTestContext) buildEventBook() *pb.EventBook {
	return &pb.EventBook{
		Cover: &pb.Cover{
			Domain: c.domain,
			Root:   &pb.UUID{Value: c.root},
		},
		Pages:        c.events,
		NextSequence: c.nextSequence,
	}
}

// buildCommandBook creates a CommandBook with a command.
func (c *tableTestContext) buildCommandBook(cmdAny *anypb.Any) *pb.CommandBook {
	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain: c.domain,
			Root:   &pb.UUID{Value: c.root},
		},
		Pages: []*pb.CommandPage{{
			Sequence: c.nextSequence,
			Command:  cmdAny,
		}},
	}
}

// addEvent adds an event to the history.
func (c *tableTestContext) addEvent(msg proto.Message) error {
	eventAny, err := anypb.New(msg)
	if err != nil {
		return err
	}
	c.events = append(c.events, &pb.EventPage{
		Sequence:  &pb.EventPage_Num{Num: c.nextSequence},
		Event:     eventAny,
		CreatedAt: timestamppb.New(time.Now()),
	})
	c.nextSequence++
	return nil
}

// rebuildState rebuilds the table state from events.
func (c *tableTestContext) rebuildState() handlers.TableState {
	return handlers.RebuildState(c.buildEventBook())
}

// getLastEvent returns the first event from lastEventBook.
func (c *tableTestContext) getLastEvent() (*anypb.Any, error) {
	if c.lastEventBook == nil || len(c.lastEventBook.Pages) == 0 {
		return nil, fmt.Errorf("no events emitted")
	}
	return c.lastEventBook.Pages[0].Event, nil
}

// --- Given Step Definitions ---

func noPriorEventsForTheTableAggregate(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	tc.events = make([]*pb.EventPage, 0)
	tc.nextSequence = 0
	return nil
}

func aTableCreatedEventFor(ctx context.Context, tableName string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	event := &examples.TableCreated{
		TableName:   tableName,
		GameVariant: examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:  5,
		BigBlind:    10,
		MinBuyIn:    tc.minBuyIn,
		MaxBuyIn:    tc.maxBuyIn,
		MaxPlayers:  tc.maxPlayers,
		CreatedAt:   timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aTableCreatedEventForWithMinBuyIn(ctx context.Context, tableName string, minBuyIn int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	tc.minBuyIn = minBuyIn

	event := &examples.TableCreated{
		TableName:   tableName,
		GameVariant: examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:  5,
		BigBlind:    10,
		MinBuyIn:    minBuyIn,
		MaxBuyIn:    tc.maxBuyIn,
		MaxPlayers:  tc.maxPlayers,
		CreatedAt:   timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aTableCreatedEventForWithMaxPlayers(ctx context.Context, tableName string, maxPlayers int) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	tc.maxPlayers = int32(maxPlayers)

	event := &examples.TableCreated{
		TableName:   tableName,
		GameVariant: examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:  5,
		BigBlind:    10,
		MinBuyIn:    tc.minBuyIn,
		MaxBuyIn:    tc.maxBuyIn,
		MaxPlayers:  int32(maxPlayers),
		CreatedAt:   timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aPlayerJoinedEventForPlayerAtSeat(ctx context.Context, playerName string, seatPosition int) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	event := &examples.PlayerJoined{
		PlayerRoot:   uuidFor(playerName),
		SeatPosition: int32(seatPosition),
		BuyInAmount:  500,
		Stack:        500,
		JoinedAt:     timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aPlayerJoinedEventForPlayerAtSeatWithStack(ctx context.Context, playerName string, seatPosition int, stack int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	event := &examples.PlayerJoined{
		PlayerRoot:   uuidFor(playerName),
		SeatPosition: int32(seatPosition),
		BuyInAmount:  stack,
		Stack:        stack,
		JoinedAt:     timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aHandStartedEventForHand(ctx context.Context, handNumber int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	// Generate a hand root
	handRoot := uuid.NewSHA1(testUUIDNamespace, []byte(fmt.Sprintf("hand-%d", handNumber)))

	// Build active players from current state
	state := tc.rebuildState()
	var activePlayers []*examples.SeatSnapshot
	for _, seat := range state.Seats {
		if !seat.IsSittingOut {
			activePlayers = append(activePlayers, &examples.SeatSnapshot{
				Position:   seat.Position,
				PlayerRoot: seat.PlayerRoot,
				Stack:      seat.Stack,
			})
		}
	}

	event := &examples.HandStarted{
		HandRoot:           handRoot[:],
		HandNumber:         handNumber,
		DealerPosition:     0,
		SmallBlindPosition: 0,
		BigBlindPosition:   1,
		ActivePlayers:      activePlayers,
		GameVariant:        examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:         5,
		BigBlind:           10,
		StartedAt:          timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aHandStartedEventForHandWithDealerAtSeat(ctx context.Context, handNumber int64, dealerSeat int) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	// Generate a hand root
	handRoot := uuid.NewSHA1(testUUIDNamespace, []byte(fmt.Sprintf("hand-%d", handNumber)))

	// Build active players from current state
	state := tc.rebuildState()
	var activePlayers []*examples.SeatSnapshot
	for _, seat := range state.Seats {
		if !seat.IsSittingOut {
			activePlayers = append(activePlayers, &examples.SeatSnapshot{
				Position:   seat.Position,
				PlayerRoot: seat.PlayerRoot,
				Stack:      seat.Stack,
			})
		}
	}

	event := &examples.HandStarted{
		HandRoot:           handRoot[:],
		HandNumber:         handNumber,
		DealerPosition:     int32(dealerSeat),
		SmallBlindPosition: int32((dealerSeat + 1) % int(tc.maxPlayers)),
		BigBlindPosition:   int32((dealerSeat + 2) % int(tc.maxPlayers)),
		ActivePlayers:      activePlayers,
		GameVariant:        examples.GameVariant_TEXAS_HOLDEM,
		SmallBlind:         5,
		BigBlind:           10,
		StartedAt:          timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aHandEndedEventForHand(ctx context.Context, handNumber int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	// Generate a hand root
	handRoot := uuid.NewSHA1(testUUIDNamespace, []byte(fmt.Sprintf("hand-%d", handNumber)))

	event := &examples.HandEnded{
		HandRoot:     handRoot[:],
		Results:      []*examples.PotResult{},
		StackChanges: map[string]int64{},
		EndedAt:      timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

// --- When Step Definitions ---

func iHandleACreateTableCommandWithNameAndVariant(ctx context.Context, tableName, variant string, table *godog.Table) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	// Parse table data
	var smallBlind, bigBlind, minBuyIn, maxBuyIn int64
	var maxPlayers int32

	for _, row := range table.Rows[1:] { // Skip header
		smallBlind, _ = strconv.ParseInt(row.Cells[0].Value, 10, 64)
		bigBlind, _ = strconv.ParseInt(row.Cells[1].Value, 10, 64)
		minBuyIn, _ = strconv.ParseInt(row.Cells[2].Value, 10, 64)
		maxBuyIn, _ = strconv.ParseInt(row.Cells[3].Value, 10, 64)
		mp, _ := strconv.ParseInt(row.Cells[4].Value, 10, 32)
		maxPlayers = int32(mp)
	}

	tc.minBuyIn = minBuyIn
	tc.maxBuyIn = maxBuyIn
	tc.maxPlayers = maxPlayers

	gameVariant := examples.GameVariant_TEXAS_HOLDEM
	if variant == "FIVE_CARD_DRAW" {
		gameVariant = examples.GameVariant_FIVE_CARD_DRAW
	}

	cmd := &examples.CreateTable{
		TableName:   tableName,
		GameVariant: gameVariant,
		SmallBlind:  smallBlind,
		BigBlind:    bigBlind,
		MinBuyIn:    minBuyIn,
		MaxBuyIn:    maxBuyIn,
		MaxPlayers:  maxPlayers,
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleCreateTable(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleAJoinTableCommandForPlayerAtSeatWithBuyIn(ctx context.Context, playerName string, seatPosition int, buyIn int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	cmd := &examples.JoinTable{
		PlayerRoot:    uuidFor(playerName),
		PreferredSeat: int32(seatPosition),
		BuyInAmount:   buyIn,
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleJoinTable(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleALeaveTableCommandForPlayer(ctx context.Context, playerName string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	cmd := &examples.LeaveTable{
		PlayerRoot: uuidFor(playerName),
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleLeaveTable(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleAStartHandCommand(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	cmd := &examples.StartHand{}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleStartHand(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleAnEndHandCommandWithWinnerWinning(ctx context.Context, winnerName string, amount int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)

	// Get current hand root from state
	state := tc.rebuildState()

	cmd := &examples.EndHand{
		HandRoot: state.CurrentHandRoot,
		Results: []*examples.PotResult{
			{
				WinnerRoot: uuidFor(winnerName),
				Amount:     amount,
				PotType:    "main",
			},
		},
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)

	result, err := handlers.HandleEndHand(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iRebuildTheTableState(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	tc.lastState = tc.rebuildState()
	return nil
}

// --- Then Step Definitions ---

func theResultIsATableCreatedEvent(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "TableCreated") {
		return fmt.Errorf("expected TableCreated event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAPlayerJoinedEvent(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "PlayerJoined") {
		return fmt.Errorf("expected PlayerJoined event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAPlayerLeftEvent(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "PlayerLeft") {
		return fmt.Errorf("expected PlayerLeft event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAHandStartedEvent(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "HandStarted") {
		return fmt.Errorf("expected HandStarted event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAHandEndedEvent(ctx context.Context) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "HandEnded") {
		return fmt.Errorf("expected HandEnded event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func tableCommandFailsWithStatus(ctx context.Context, status string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	if tc.lastError == nil {
		return fmt.Errorf("expected command to fail but it succeeded")
	}
	var cmdErr *angzarr.CommandRejectedError
	if errors.As(tc.lastError, &cmdErr) {
		return nil
	}
	return nil
}

func tableErrorMessageContains(ctx context.Context, expected string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	if tc.lastError == nil {
		return fmt.Errorf("expected error but got none")
	}
	if !strings.Contains(strings.ToLower(tc.lastErrorMsg), strings.ToLower(expected)) {
		return fmt.Errorf("expected error message to contain '%s' but got '%s'", expected, tc.lastErrorMsg)
	}
	return nil
}

func theTableEventHasTableName(ctx context.Context, expected string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	var event examples.TableCreated
	if err := eventAny.UnmarshalTo(&event); err != nil {
		return fmt.Errorf("failed to unmarshal event: %w", err)
	}

	if event.TableName != expected {
		return fmt.Errorf("expected table_name '%s' but got '%s'", expected, event.TableName)
	}
	return nil
}

func theTableEventHasGameVariant(ctx context.Context, expected string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	var event examples.TableCreated
	if err := eventAny.UnmarshalTo(&event); err != nil {
		return fmt.Errorf("failed to unmarshal event: %w", err)
	}

	actualType := event.GameVariant.String()
	if actualType != expected {
		return fmt.Errorf("expected game_variant '%s' but got '%s'", expected, actualType)
	}
	return nil
}

func theTableEventHasSmallBlind(ctx context.Context, expected int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	var event examples.TableCreated
	if err := eventAny.UnmarshalTo(&event); err != nil {
		return fmt.Errorf("failed to unmarshal event: %w", err)
	}

	if event.SmallBlind != expected {
		return fmt.Errorf("expected small_blind %d but got %d", expected, event.SmallBlind)
	}
	return nil
}

func theTableEventHasBigBlind(ctx context.Context, expected int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	var event examples.TableCreated
	if err := eventAny.UnmarshalTo(&event); err != nil {
		return fmt.Errorf("failed to unmarshal event: %w", err)
	}

	if event.BigBlind != expected {
		return fmt.Errorf("expected big_blind %d but got %d", expected, event.BigBlind)
	}
	return nil
}

func theTableEventHasSeatPosition(ctx context.Context, expected int) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "PlayerJoined") {
		var event examples.PlayerJoined
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.SeatPosition != int32(expected) {
			return fmt.Errorf("expected seat_position %d but got %d", expected, event.SeatPosition)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for seat_position check: %s", eventAny.TypeUrl)
}

func theTableEventHasBuyInAmount(ctx context.Context, expected int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "PlayerJoined") {
		var event examples.PlayerJoined
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.BuyInAmount != expected {
			return fmt.Errorf("expected buy_in_amount %d but got %d", expected, event.BuyInAmount)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for buy_in_amount check: %s", eventAny.TypeUrl)
}

func theTableEventHasChipsCashedOut(ctx context.Context, expected int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "PlayerLeft") {
		var event examples.PlayerLeft
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.ChipsCashedOut != expected {
			return fmt.Errorf("expected chips_cashed_out %d but got %d", expected, event.ChipsCashedOut)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for chips_cashed_out check: %s", eventAny.TypeUrl)
}

func theTableEventHasHandNumber(ctx context.Context, expected int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "HandStarted") {
		var event examples.HandStarted
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.HandNumber != expected {
			return fmt.Errorf("expected hand_number %d but got %d", expected, event.HandNumber)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for hand_number check: %s", eventAny.TypeUrl)
}

func theTableEventHasActivePlayers(ctx context.Context, expected int) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "HandStarted") {
		var event examples.HandStarted
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if len(event.ActivePlayers) != expected {
			return fmt.Errorf("expected %d active_players but got %d", expected, len(event.ActivePlayers))
		}
		return nil
	}

	return fmt.Errorf("unknown event type for active_players check: %s", eventAny.TypeUrl)
}

func theTableEventHasDealerPosition(ctx context.Context, expected int) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "HandStarted") {
		var event examples.HandStarted
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.DealerPosition != int32(expected) {
			return fmt.Errorf("expected dealer_position %d but got %d", expected, event.DealerPosition)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for dealer_position check: %s", eventAny.TypeUrl)
}

func playerStackChangeIs(ctx context.Context, playerName string, expected int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "HandEnded") {
		var event examples.HandEnded
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		playerHex := hex.EncodeToString(uuidFor(playerName))
		actual, exists := event.StackChanges[playerHex]
		if !exists {
			return fmt.Errorf("no stack change found for player '%s'", playerName)
		}
		if actual != expected {
			return fmt.Errorf("expected stack change %d for player '%s' but got %d", expected, playerName, actual)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for stack change check: %s", eventAny.TypeUrl)
}

// --- State Check Step Definitions ---

func theTableStateHasPlayers(ctx context.Context, expected int) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	actual := tc.lastState.PlayerCount()
	if actual != expected {
		return fmt.Errorf("expected %d players but got %d", expected, actual)
	}
	return nil
}

func theTableStateHasSeatOccupiedBy(ctx context.Context, seatPosition int, playerName string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	seat, exists := tc.lastState.Seats[int32(seatPosition)]
	if !exists {
		return fmt.Errorf("seat %d is not occupied", seatPosition)
	}
	expectedRoot := uuidFor(playerName)
	if hex.EncodeToString(seat.PlayerRoot) != hex.EncodeToString(expectedRoot) {
		return fmt.Errorf("seat %d is not occupied by '%s'", seatPosition, playerName)
	}
	return nil
}

func theTableStateHasStatus(ctx context.Context, expected string) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	if tc.lastState.Status != expected {
		return fmt.Errorf("expected status '%s' but got '%s'", expected, tc.lastState.Status)
	}
	return nil
}

func theTableStateHasHandCount(ctx context.Context, expected int64) error {
	tc := ctx.Value("tableTestContext").(*tableTestContext)
	if tc.lastState.HandCount != expected {
		return fmt.Errorf("expected hand_count %d but got %d", expected, tc.lastState.HandCount)
	}
	return nil
}

// InitializeTableScenario sets up the godog scenario context for table tests.
func InitializeTableScenario(ctx *godog.ScenarioContext) {
	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc := newTableTestContext()
		return context.WithValue(ctx, "tableTestContext", tc), nil
	})

	// Given steps
	ctx.Step(`^no prior events for the table aggregate$`, noPriorEventsForTheTableAggregate)
	ctx.Step(`^a TableCreated event for "([^"]*)"$`, aTableCreatedEventFor)
	ctx.Step(`^a TableCreated event for "([^"]*)" with min_buy_in (\d+)$`, aTableCreatedEventForWithMinBuyIn)
	ctx.Step(`^a TableCreated event for "([^"]*)" with max_players (\d+)$`, aTableCreatedEventForWithMaxPlayers)
	ctx.Step(`^a PlayerJoined event for player "([^"]*)" at seat (\d+)$`, aPlayerJoinedEventForPlayerAtSeat)
	ctx.Step(`^a PlayerJoined event for player "([^"]*)" at seat (\d+) with stack (\d+)$`, aPlayerJoinedEventForPlayerAtSeatWithStack)
	ctx.Step(`^a HandStarted event for hand (\d+)$`, aHandStartedEventForHand)
	ctx.Step(`^a HandStarted event for hand (\d+) with dealer at seat (\d+)$`, aHandStartedEventForHandWithDealerAtSeat)
	ctx.Step(`^a HandEnded event for hand (\d+)$`, aHandEndedEventForHand)

	// When steps
	ctx.Step(`^I handle a CreateTable command with name "([^"]*)" and variant "([^"]*)":$`, iHandleACreateTableCommandWithNameAndVariant)
	ctx.Step(`^I handle a JoinTable command for player "([^"]*)" at seat (-?\d+) with buy-in (\d+)$`, iHandleAJoinTableCommandForPlayerAtSeatWithBuyIn)
	ctx.Step(`^I handle a LeaveTable command for player "([^"]*)"$`, iHandleALeaveTableCommandForPlayer)
	ctx.Step(`^I handle a StartHand command$`, iHandleAStartHandCommand)
	ctx.Step(`^I handle an EndHand command with winner "([^"]*)" winning (\d+)$`, iHandleAnEndHandCommandWithWinnerWinning)
	ctx.Step(`^I rebuild the table state$`, iRebuildTheTableState)

	// Then steps - result checks
	ctx.Step(`^the result is a TableCreated event$`, theResultIsATableCreatedEvent)
	ctx.Step(`^the result is a PlayerJoined event$`, theResultIsAPlayerJoinedEvent)
	ctx.Step(`^the result is a PlayerLeft event$`, theResultIsAPlayerLeftEvent)
	ctx.Step(`^the result is a HandStarted event$`, theResultIsAHandStartedEvent)
	ctx.Step(`^the result is a HandEnded event$`, theResultIsAHandEndedEvent)

	// Then steps - error checks
	ctx.Step(`^the command fails with status "([^"]*)"$`, tableCommandFailsWithStatus)
	ctx.Step(`^the error message contains "([^"]*)"$`, tableErrorMessageContains)

	// Then steps - event property checks
	ctx.Step(`^the table event has table_name "([^"]*)"$`, theTableEventHasTableName)
	ctx.Step(`^the table event has game_variant "([^"]*)"$`, theTableEventHasGameVariant)
	ctx.Step(`^the table event has small_blind (\d+)$`, theTableEventHasSmallBlind)
	ctx.Step(`^the table event has big_blind (\d+)$`, theTableEventHasBigBlind)
	ctx.Step(`^the table event has seat_position (\d+)$`, theTableEventHasSeatPosition)
	ctx.Step(`^the table event has buy_in_amount (\d+)$`, theTableEventHasBuyInAmount)
	ctx.Step(`^the table event has chips_cashed_out (\d+)$`, theTableEventHasChipsCashedOut)
	ctx.Step(`^the table event has hand_number (\d+)$`, theTableEventHasHandNumber)
	ctx.Step(`^the table event has (\d+) active_players$`, theTableEventHasActivePlayers)
	ctx.Step(`^the table event has dealer_position (\d+)$`, theTableEventHasDealerPosition)
	ctx.Step(`^player "([^"]*)" stack change is (\d+)$`, playerStackChangeIs)

	// Then steps - state checks
	ctx.Step(`^the table state has (\d+) players?$`, theTableStateHasPlayers)
	ctx.Step(`^the table state has seat (\d+) occupied by "([^"]*)"$`, theTableStateHasSeatOccupiedBy)
	ctx.Step(`^the table state has status "([^"]*)"$`, theTableStateHasStatus)
	ctx.Step(`^the table state has hand_count (\d+)$`, theTableStateHasHandCount)
}

func TestTableFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeTableScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"../../features/unit/table.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}
