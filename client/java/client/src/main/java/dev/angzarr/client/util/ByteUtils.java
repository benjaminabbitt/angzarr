package dev.angzarr.client.util;

/**
 * Utility methods for byte array operations.
 */
public final class ByteUtils {

    private ByteUtils() {
        // Utility class
    }

    /**
     * Converts a byte array to a hexadecimal string.
     *
     * @param bytes the byte array to convert
     * @return hexadecimal string representation, or empty string if bytes is null
     */
    public static String bytesToHex(byte[] bytes) {
        if (bytes == null) return "";
        StringBuilder sb = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }

    /**
     * Converts a byte array to a long value (up to 8 bytes).
     *
     * @param bytes the byte array to convert
     * @return long value
     */
    public static long bytesToLong(byte[] bytes) {
        if (bytes == null) return 0;
        long result = 0;
        for (int i = 0; i < Math.min(8, bytes.length); i++) {
            result = (result << 8) | (bytes[i] & 0xFF);
        }
        return result;
    }
}
