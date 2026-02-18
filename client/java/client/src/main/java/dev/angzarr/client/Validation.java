package dev.angzarr.client;

import java.util.Collection;

/**
 * Validation helper methods that throw CommandRejectedError on failure.
 */
public final class Validation {

    private Validation() {}

    /**
     * Require that an aggregate exists (has prior events).
     */
    public static void requireExists(boolean exists, String message) {
        if (!exists) {
            throw new Errors.CommandRejectedError(message);
        }
    }

    public static void requireExists(boolean exists) {
        requireExists(exists, "Aggregate does not exist");
    }

    /**
     * Require that an aggregate does not exist.
     */
    public static void requireNotExists(boolean exists, String message) {
        if (exists) {
            throw new Errors.CommandRejectedError(message);
        }
    }

    public static void requireNotExists(boolean exists) {
        requireNotExists(exists, "Aggregate already exists");
    }

    /**
     * Require that a value is positive (greater than zero).
     */
    public static void requirePositive(long value, String fieldName) {
        if (value <= 0) {
            throw new Errors.CommandRejectedError(fieldName + " must be positive");
        }
    }

    public static void requirePositive(double value, String fieldName) {
        if (value <= 0) {
            throw new Errors.CommandRejectedError(fieldName + " must be positive");
        }
    }

    /**
     * Require that a value is non-negative (zero or greater).
     */
    public static void requireNonNegative(long value, String fieldName) {
        if (value < 0) {
            throw new Errors.CommandRejectedError(fieldName + " must be non-negative");
        }
    }

    public static void requireNonNegative(double value, String fieldName) {
        if (value < 0) {
            throw new Errors.CommandRejectedError(fieldName + " must be non-negative");
        }
    }

    /**
     * Require that a string is not empty.
     */
    public static void requireNotEmpty(String value, String fieldName) {
        if (value == null || value.isEmpty()) {
            throw new Errors.CommandRejectedError(fieldName + " must not be empty");
        }
    }

    /**
     * Require that a collection is not empty.
     */
    public static void requireNotEmpty(Collection<?> collection, String fieldName) {
        if (collection == null || collection.isEmpty()) {
            throw new Errors.CommandRejectedError(fieldName + " must not be empty");
        }
    }

    /**
     * Require that a status matches an expected value.
     */
    public static <T extends Enum<T>> void requireStatus(T actual, T expected, String message) {
        if (!actual.equals(expected)) {
            throw new Errors.CommandRejectedError(message + ": expected " + expected + ", got " + actual);
        }
    }

    /**
     * Require that a status does not match a forbidden value.
     */
    public static <T extends Enum<T>> void requireStatusNot(T actual, T forbidden, String message) {
        if (actual.equals(forbidden)) {
            throw new Errors.CommandRejectedError(message + ": must not be " + forbidden);
        }
    }
}
