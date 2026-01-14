package dev.angzarr.integration;

import io.grpc.ManagedChannel;
import dev.angzarr.Angzarr.EventBook;
import dev.angzarr.Angzarr.CommandResponse;

import java.util.UUID;

public class TestContext {
    private ManagedChannel channel;
    private String angzarrHost;
    private int angzarrPort;

    private UUID currentCustomerId;
    private UUID currentTransactionId;
    private CommandResponse lastResponse;
    private EventBook lastEventBook;
    private Exception lastException;

    public ManagedChannel getChannel() {
        return channel;
    }

    public void setChannel(ManagedChannel channel) {
        this.channel = channel;
    }

    public String getAngzarrHost() {
        return angzarrHost;
    }

    public void setAngzarrHost(String angzarrHost) {
        this.angzarrHost = angzarrHost;
    }

    public int getAngzarrPort() {
        return angzarrPort;
    }

    public void setAngzarrPort(int angzarrPort) {
        this.angzarrPort = angzarrPort;
    }

    public UUID getCurrentCustomerId() {
        return currentCustomerId;
    }

    public void setCurrentCustomerId(UUID currentCustomerId) {
        this.currentCustomerId = currentCustomerId;
    }

    public UUID getCurrentTransactionId() {
        return currentTransactionId;
    }

    public void setCurrentTransactionId(UUID currentTransactionId) {
        this.currentTransactionId = currentTransactionId;
    }

    public CommandResponse getLastResponse() {
        return lastResponse;
    }

    public void setLastResponse(CommandResponse lastResponse) {
        this.lastResponse = lastResponse;
    }

    public EventBook getLastEventBook() {
        return lastEventBook;
    }

    public void setLastEventBook(EventBook lastEventBook) {
        this.lastEventBook = lastEventBook;
    }

    public Exception getLastException() {
        return lastException;
    }

    public void setLastException(Exception lastException) {
        this.lastException = lastException;
    }

    public void reset() {
        this.currentCustomerId = null;
        this.currentTransactionId = null;
        this.lastResponse = null;
        this.lastEventBook = null;
        this.lastException = null;
    }
}
