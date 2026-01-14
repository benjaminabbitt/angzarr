package dev.angzarr.examples.customer;

/**
 * Represents the current state of a customer aggregate.
 */
public record CustomerState(
    String name,
    String email,
    int loyaltyPoints,
    int lifetimePoints
) {
    public static CustomerState empty() {
        return new CustomerState("", "", 0, 0);
    }

    public boolean exists() {
        return !name.isEmpty();
    }

    public CustomerState withName(String name) {
        return new CustomerState(name, this.email, this.loyaltyPoints, this.lifetimePoints);
    }

    public CustomerState withEmail(String email) {
        return new CustomerState(this.name, email, this.loyaltyPoints, this.lifetimePoints);
    }

    public CustomerState withLoyaltyPoints(int loyaltyPoints) {
        return new CustomerState(this.name, this.email, loyaltyPoints, this.lifetimePoints);
    }

    public CustomerState addLifetimePoints(int points) {
        return new CustomerState(this.name, this.email, this.loyaltyPoints, this.lifetimePoints + points);
    }
}
