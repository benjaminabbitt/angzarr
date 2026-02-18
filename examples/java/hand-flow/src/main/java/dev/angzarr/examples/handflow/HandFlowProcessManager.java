package dev.angzarr.examples.handflow;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.*;
import dev.angzarr.examples.*;

import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/**
 * Process Manager: Hand Flow Orchestration
 *
 * <p>Orchestrates the flow of a poker hand by:
 * 1. Subscribing to table and hand domain events
 * 2. Managing hand process state machines
 * 3. Sending commands to drive hands forward
 */
public class HandFlowProcessManager {

    private final Map<String, HandProcess> processes = new HashMap<>();
    private final Consumer<CommandBook> commandSender;

    public HandFlowProcessManager(Consumer<CommandBook> commandSender) {
        this.commandSender = commandSender;
    }

    /**
     * Get list of domains this PM subscribes to.
     */
    public List<String> getInputDomains() {
        return List.of("table", "hand");
    }

    /**
     * Phase 1: Declare additional destinations needed.
     */
    public List<Cover> prepare(EventBook trigger, EventBook processState) {
        List<Cover> destinations = new ArrayList<>();

        for (EventPage page : trigger.getPagesList()) {
            String typeUrl = page.getEvent().getTypeUrl();
            try {
                if (typeUrl.endsWith("HandStarted")) {
                    HandStarted event = page.getEvent().unpack(HandStarted.class);
                    destinations.add(Cover.newBuilder()
                        .setDomain("hand")
                        .setRoot(dev.angzarr.UUID.newBuilder().setValue(event.getHandRoot()))
                        .build());
                }
            } catch (InvalidProtocolBufferException e) {
                throw new RuntimeException("Failed to unpack event: " + typeUrl, e);
            }
        }

        return destinations;
    }

    /**
     * Phase 2: Process events and produce commands.
     */
    public List<CommandBook> handle(EventBook trigger, EventBook processState, List<EventBook> destinations) {
        List<CommandBook> commands = new ArrayList<>();

        for (EventPage page : trigger.getPagesList()) {
            Any eventAny = page.getEvent();
            String typeUrl = eventAny.getTypeUrl();

            try {
                if (typeUrl.endsWith("HandStarted")) {
                    HandStarted event = eventAny.unpack(HandStarted.class);
                    CommandBook cmd = handleHandStarted(event);
                    if (cmd != null) commands.add(cmd);

                } else if (typeUrl.endsWith("CardsDealt")) {
                    CardsDealt event = eventAny.unpack(CardsDealt.class);
                    CommandBook cmd = handleCardsDealt(event);
                    if (cmd != null) commands.add(cmd);

                } else if (typeUrl.endsWith("BlindPosted")) {
                    BlindPosted event = eventAny.unpack(BlindPosted.class);
                    CommandBook cmd = handleBlindPosted(event);
                    if (cmd != null) commands.add(cmd);

                } else if (typeUrl.endsWith("ActionTaken")) {
                    ActionTaken event = eventAny.unpack(ActionTaken.class);
                    CommandBook cmd = handleActionTaken(event);
                    if (cmd != null) commands.add(cmd);

                } else if (typeUrl.endsWith("CommunityCardsDealt")) {
                    CommunityCardsDealt event = eventAny.unpack(CommunityCardsDealt.class);
                    CommandBook cmd = handleCommunityCardsDealt(event);
                    if (cmd != null) commands.add(cmd);

                } else if (typeUrl.endsWith("PotAwarded")) {
                    PotAwarded event = eventAny.unpack(PotAwarded.class);
                    handlePotAwarded(event);
                }
            } catch (InvalidProtocolBufferException e) {
                throw new RuntimeException("Failed to unpack event: " + typeUrl, e);
            }
        }

        return commands;
    }

    /**
     * Initialize process for a new hand.
     */
    private CommandBook handleHandStarted(HandStarted event) {
        byte[] tableRoot = event.getHandRoot().toByteArray();
        String handId = bytesToHex(tableRoot) + "_" + event.getHandNumber();

        HandProcess process = new HandProcess();
        process.setHandId(handId);
        process.setTableRoot(tableRoot);
        process.setHandNumber(event.getHandNumber());
        process.setGameVariant(event.getGameVariantValue());
        process.setDealerPosition(event.getDealerPosition());
        process.setSmallBlindPosition(event.getSmallBlindPosition());
        process.setBigBlindPosition(event.getBigBlindPosition());
        process.setSmallBlind(event.getSmallBlind());
        process.setBigBlind(event.getBigBlind());
        process.setPhase(HandPhase.DEALING);

        // Initialize player states
        for (SeatSnapshot player : event.getActivePlayersList()) {
            process.getPlayers().put(player.getPosition(), new PlayerProcessState(
                player.getPlayerRoot().toByteArray(),
                player.getPosition(),
                player.getStack()
            ));
            process.getActivePositions().add(player.getPosition());
        }

        Collections.sort(process.getActivePositions());
        processes.put(handId, process);

        return null; // No immediate command needed, DealCards comes from saga
    }

    /**
     * Handle CardsDealt - transition to blind posting.
     */
    private CommandBook handleCardsDealt(CardsDealt event) {
        String handId = bytesToHex(event.getTableRoot().toByteArray()) + "_" + event.getHandNumber();
        HandProcess process = processes.get(handId);
        if (process == null) return null;

        process.setPhase(HandPhase.POSTING_BLINDS);
        process.setMinRaise(process.getBigBlind());

        return buildPostBlindCommand(process);
    }

    /**
     * Handle BlindPosted - continue blind posting or start betting.
     */
    private CommandBook handleBlindPosted(BlindPosted event) {
        HandProcess process = findProcessByPlayer(event.getPlayerRoot().toByteArray());
        if (process == null) return null;

        // Update player state
        for (PlayerProcessState player : process.getPlayers().values()) {
            if (Arrays.equals(player.getPlayerRoot(), event.getPlayerRoot().toByteArray())) {
                player.setStack(event.getPlayerStack());
                player.setBetThisRound(event.getAmount());
                player.setTotalInvested(event.getAmount());
                break;
            }
        }

        process.setPotTotal(event.getPotTotal());

        if ("small".equals(event.getBlindType())) {
            process.setSmallBlindPosted(true);
            process.setCurrentBet(event.getAmount());
            return buildPostBlindCommand(process);
        } else if ("big".equals(event.getBlindType())) {
            process.setBigBlindPosted(true);
            process.setCurrentBet(event.getAmount());
            return startBetting(process);
        }

        return null;
    }

    /**
     * Handle ActionTaken - advance to next player or phase.
     */
    private CommandBook handleActionTaken(ActionTaken event) {
        HandProcess process = findProcessByPlayer(event.getPlayerRoot().toByteArray());
        if (process == null) return null;

        // Update player state
        for (PlayerProcessState player : process.getPlayers().values()) {
            if (Arrays.equals(player.getPlayerRoot(), event.getPlayerRoot().toByteArray())) {
                player.setStack(event.getPlayerStack());
                player.setHasActed(true);

                if (event.getAction() == ActionType.FOLD) {
                    player.setHasFolded(true);
                } else if (event.getAction() == ActionType.ALL_IN) {
                    player.setAllIn(true);
                    player.setBetThisRound(player.getBetThisRound() + event.getAmount());
                    player.setTotalInvested(player.getTotalInvested() + event.getAmount());
                } else if (event.getAction() == ActionType.CALL ||
                           event.getAction() == ActionType.BET ||
                           event.getAction() == ActionType.RAISE) {
                    player.setBetThisRound(player.getBetThisRound() + event.getAmount());
                    player.setTotalInvested(player.getTotalInvested() + event.getAmount());
                }

                if (event.getAction() == ActionType.BET ||
                    event.getAction() == ActionType.RAISE ||
                    event.getAction() == ActionType.ALL_IN) {
                    if (player.getBetThisRound() > process.getCurrentBet()) {
                        long raiseAmount = player.getBetThisRound() - process.getCurrentBet();
                        process.setCurrentBet(player.getBetThisRound());
                        process.setMinRaise(Math.max(process.getMinRaise(), raiseAmount));
                        process.setLastAggressor(player.getPosition());
                        // Reset has_acted for other active players
                        for (PlayerProcessState p : process.getPlayers().values()) {
                            if (p.getPosition() != player.getPosition() &&
                                !p.hasFolded() && !p.isAllIn()) {
                                p.setHasActed(false);
                            }
                        }
                    }
                }
                break;
            }
        }

        process.setPotTotal(event.getPotTotal());

        // Check if betting round is complete
        if (isBettingComplete(process)) {
            return endBettingRound(process);
        } else {
            // Betting continues - no command needed, player action requested externally
            advanceAction(process);
            return null;
        }
    }

    /**
     * Handle CommunityCardsDealt - start new betting round.
     */
    private CommandBook handleCommunityCardsDealt(CommunityCardsDealt event) {
        // Find process - need to track by community card events
        // For simplicity, assume single active process
        for (HandProcess process : processes.values()) {
            if (process.getPhase() == HandPhase.DEALING_COMMUNITY) {
                process.setCommunityCardCount(event.getAllCommunityCardsCount());
                process.setBettingPhase(event.getPhaseValue());
                return startBetting(process);
            }
        }
        return null;
    }

    /**
     * Handle PotAwarded - hand is complete.
     */
    private void handlePotAwarded(PotAwarded event) {
        for (HandProcess process : processes.values()) {
            if (process.getPhase() != HandPhase.COMPLETE) {
                process.setPhase(HandPhase.COMPLETE);
            }
        }
    }

    // --- Helper methods ---

    private CommandBook buildPostBlindCommand(HandProcess process) {
        PlayerProcessState player;
        String blindType;
        long amount;

        if (!process.isSmallBlindPosted()) {
            player = process.getPlayers().get(process.getSmallBlindPosition());
            blindType = "small";
            amount = process.getSmallBlind();
        } else if (!process.isBigBlindPosted()) {
            player = process.getPlayers().get(process.getBigBlindPosition());
            blindType = "big";
            amount = process.getBigBlind();
        } else {
            return null;
        }

        if (player == null) return null;

        PostBlind cmd = PostBlind.newBuilder()
            .setPlayerRoot(ByteString.copyFrom(player.getPlayerRoot()))
            .setBlindType(blindType)
            .setAmount(amount)
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("hand")
                .setRoot(dev.angzarr.UUID.newBuilder().setValue(ByteString.copyFrom(process.getTableRoot()))))
            .addPages(CommandPage.newBuilder()
                .setCommand(Any.pack(cmd, "type.googleapis.com/")))
            .build();
    }

    private CommandBook startBetting(HandProcess process) {
        process.setPhase(HandPhase.BETTING);

        // Reset betting state for new round
        for (PlayerProcessState player : process.getPlayers().values()) {
            player.setBetThisRound(0);
            player.setHasActed(false);
        }
        process.setCurrentBet(0);

        // Determine first to act
        if (process.getBettingPhase() == BettingPhase.PREFLOP_VALUE) {
            process.setActionOn(findNextActive(process, process.getBigBlindPosition()));
        } else {
            process.setActionOn(findNextActive(process, process.getDealerPosition()));
        }

        process.setActionStartedAt(Instant.now());
        return null; // Player action requests handled externally
    }

    private void advanceAction(HandProcess process) {
        process.setActionOn(findNextActive(process, process.getActionOn()));
        process.setActionStartedAt(Instant.now());
    }

    private int findNextActive(HandProcess process, int afterPosition) {
        List<Integer> positions = process.getActivePositions();
        int n = positions.size();
        if (n == 0) return -1;

        int startIdx = 0;
        for (int i = 0; i < n; i++) {
            if (positions.get(i) > afterPosition) {
                startIdx = i;
                break;
            }
        }

        for (int i = 0; i < n; i++) {
            int idx = (startIdx + i) % n;
            int pos = positions.get(idx);
            PlayerProcessState player = process.getPlayers().get(pos);
            if (player != null && !player.hasFolded() && !player.isAllIn()) {
                return pos;
            }
        }

        return -1;
    }

    private boolean isBettingComplete(HandProcess process) {
        List<PlayerProcessState> activePlayers = new ArrayList<>();
        for (PlayerProcessState p : process.getPlayers().values()) {
            if (!p.hasFolded() && !p.isAllIn()) {
                activePlayers.add(p);
            }
        }

        if (activePlayers.size() <= 1) return true;

        for (PlayerProcessState player : activePlayers) {
            if (!player.hasActed()) return false;
            if (player.getBetThisRound() < process.getCurrentBet() && !player.isAllIn()) {
                return false;
            }
        }

        return true;
    }

    private CommandBook endBettingRound(HandProcess process) {
        // Count players in hand
        List<PlayerProcessState> playersInHand = new ArrayList<>();
        List<PlayerProcessState> activePlayers = new ArrayList<>();
        for (PlayerProcessState p : process.getPlayers().values()) {
            if (!p.hasFolded()) {
                playersInHand.add(p);
                if (!p.isAllIn()) {
                    activePlayers.add(p);
                }
            }
        }

        // If only one player left, award pot
        if (playersInHand.size() == 1) {
            return awardPotToLastPlayer(process, playersInHand.get(0));
        }

        // Advance to next phase
        return advancePhase(process, activePlayers.size());
    }

    private CommandBook advancePhase(HandProcess process, int activeCount) {
        int currentPhase = process.getBettingPhase();

        if (currentPhase == BettingPhase.PREFLOP_VALUE) {
            process.setPhase(HandPhase.DEALING_COMMUNITY);
            return buildDealCommunityCommand(process, 3); // Flop
        } else if (currentPhase == BettingPhase.FLOP_VALUE) {
            process.setPhase(HandPhase.DEALING_COMMUNITY);
            return buildDealCommunityCommand(process, 1); // Turn
        } else if (currentPhase == BettingPhase.TURN_VALUE) {
            process.setPhase(HandPhase.DEALING_COMMUNITY);
            return buildDealCommunityCommand(process, 1); // River
        } else if (currentPhase == BettingPhase.RIVER_VALUE) {
            return autoAwardPot(process);
        }

        return null;
    }

    private CommandBook buildDealCommunityCommand(HandProcess process, int count) {
        DealCommunityCards cmd = DealCommunityCards.newBuilder()
            .setCount(count)
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("hand")
                .setRoot(dev.angzarr.UUID.newBuilder().setValue(ByteString.copyFrom(process.getTableRoot()))))
            .addPages(CommandPage.newBuilder()
                .setCommand(Any.pack(cmd, "type.googleapis.com/")))
            .build();
    }

    private CommandBook awardPotToLastPlayer(HandProcess process, PlayerProcessState winner) {
        process.setPhase(HandPhase.COMPLETE);

        AwardPot cmd = AwardPot.newBuilder()
            .addAwards(PotAward.newBuilder()
                .setPlayerRoot(ByteString.copyFrom(winner.getPlayerRoot()))
                .setAmount(process.getPotTotal())
                .setPotType("main"))
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("hand")
                .setRoot(dev.angzarr.UUID.newBuilder().setValue(ByteString.copyFrom(process.getTableRoot()))))
            .addPages(CommandPage.newBuilder()
                .setCommand(Any.pack(cmd, "type.googleapis.com/")))
            .build();
    }

    private CommandBook autoAwardPot(HandProcess process) {
        List<PlayerProcessState> playersInHand = new ArrayList<>();
        for (PlayerProcessState p : process.getPlayers().values()) {
            if (!p.hasFolded()) {
                playersInHand.add(p);
            }
        }

        if (playersInHand.isEmpty()) return null;

        // Split pot evenly (simplified - real implementation evaluates hands)
        long split = process.getPotTotal() / playersInHand.size();
        long remainder = process.getPotTotal() % playersInHand.size();

        AwardPot.Builder cmdBuilder = AwardPot.newBuilder();
        for (int i = 0; i < playersInHand.size(); i++) {
            PlayerProcessState player = playersInHand.get(i);
            long amount = split + (i < remainder ? 1 : 0);
            cmdBuilder.addAwards(PotAward.newBuilder()
                .setPlayerRoot(ByteString.copyFrom(player.getPlayerRoot()))
                .setAmount(amount)
                .setPotType("main"));
        }

        process.setPhase(HandPhase.COMPLETE);

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("hand")
                .setRoot(dev.angzarr.UUID.newBuilder().setValue(ByteString.copyFrom(process.getTableRoot()))))
            .addPages(CommandPage.newBuilder()
                .setCommand(Any.pack(cmdBuilder.build(), "type.googleapis.com/")))
            .build();
    }

    private HandProcess findProcessByPlayer(byte[] playerRoot) {
        for (HandProcess process : processes.values()) {
            for (PlayerProcessState player : process.getPlayers().values()) {
                if (Arrays.equals(player.getPlayerRoot(), playerRoot)) {
                    return process;
                }
            }
        }
        return null;
    }

    private static byte[] hexToBytes(String hex) {
        int len = hex.length();
        byte[] data = new byte[len / 2];
        for (int i = 0; i < len; i += 2) {
            data[i / 2] = (byte) ((Character.digit(hex.charAt(i), 16) << 4)
                + Character.digit(hex.charAt(i + 1), 16));
        }
        return data;
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
