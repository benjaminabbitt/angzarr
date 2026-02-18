namespace Angzarr.Client;

/// <summary>
/// Validation helper methods that throw CommandRejectedError on failure.
/// </summary>
public static class Validation
{
    /// <summary>
    /// Require that an aggregate exists (has prior events).
    /// </summary>
    public static void RequireExists(bool exists, string message = "Aggregate does not exist")
    {
        if (!exists)
            throw new CommandRejectedError(message);
    }

    /// <summary>
    /// Require that an aggregate does not exist.
    /// </summary>
    public static void RequireNotExists(bool exists, string message = "Aggregate already exists")
    {
        if (exists)
            throw new CommandRejectedError(message);
    }

    /// <summary>
    /// Require that a value is positive (greater than zero).
    /// </summary>
    public static void RequirePositive(decimal value, string fieldName = "value")
    {
        if (value <= 0)
            throw new CommandRejectedError($"{fieldName} must be positive");
    }

    /// <summary>
    /// Require that a value is positive (greater than zero).
    /// </summary>
    public static void RequirePositive(int value, string fieldName = "value")
    {
        if (value <= 0)
            throw new CommandRejectedError($"{fieldName} must be positive");
    }

    /// <summary>
    /// Require that a value is positive (greater than zero).
    /// </summary>
    public static void RequirePositive(long value, string fieldName = "value")
    {
        if (value <= 0)
            throw new CommandRejectedError($"{fieldName} must be positive");
    }

    /// <summary>
    /// Require that a value is non-negative (zero or greater).
    /// </summary>
    public static void RequireNonNegative(decimal value, string fieldName = "value")
    {
        if (value < 0)
            throw new CommandRejectedError($"{fieldName} must be non-negative");
    }

    /// <summary>
    /// Require that a value is non-negative (zero or greater).
    /// </summary>
    public static void RequireNonNegative(int value, string fieldName = "value")
    {
        if (value < 0)
            throw new CommandRejectedError($"{fieldName} must be non-negative");
    }

    /// <summary>
    /// Require that a value is non-negative (zero or greater).
    /// </summary>
    public static void RequireNonNegative(long value, string fieldName = "value")
    {
        if (value < 0)
            throw new CommandRejectedError($"{fieldName} must be non-negative");
    }

    /// <summary>
    /// Require that a string is not empty.
    /// </summary>
    public static void RequireNotEmpty(string? value, string fieldName = "value")
    {
        if (string.IsNullOrEmpty(value))
            throw new CommandRejectedError($"{fieldName} must not be empty");
    }

    /// <summary>
    /// Require that a collection is not empty.
    /// </summary>
    public static void RequireNotEmpty<T>(IEnumerable<T>? collection, string fieldName = "collection")
    {
        if (collection == null || !collection.Any())
            throw new CommandRejectedError($"{fieldName} must not be empty");
    }

    /// <summary>
    /// Require that a status matches an expected value.
    /// </summary>
    public static void RequireStatus<T>(T actual, T expected, string message = "Invalid status")
        where T : struct, Enum
    {
        if (!actual.Equals(expected))
            throw new CommandRejectedError($"{message}: expected {expected}, got {actual}");
    }

    /// <summary>
    /// Require that a status does not match a forbidden value.
    /// </summary>
    public static void RequireStatusNot<T>(T actual, T forbidden, string message = "Invalid status")
        where T : struct, Enum
    {
        if (actual.Equals(forbidden))
            throw new CommandRejectedError($"{message}: must not be {forbidden}");
    }
}
