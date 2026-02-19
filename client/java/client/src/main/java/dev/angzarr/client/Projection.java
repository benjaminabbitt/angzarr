package dev.angzarr.client;

/**
 * Projection result from a projector handler.
 */
public class Projection {
    private final String key;
    private final String value;
    private final boolean isDelete;

    private Projection(String key, String value, boolean isDelete) {
        this.key = key;
        this.value = value;
        this.isDelete = isDelete;
    }

    public static Projection upsert(String key, String value) {
        return new Projection(key, value, false);
    }

    public static Projection delete(String key) {
        return new Projection(key, "", true);
    }

    public String getKey() {
        return key;
    }

    public String getValue() {
        return value;
    }

    public boolean isDelete() {
        return isDelete;
    }
}
