package dev.angzarr.client.annotations;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a method as a CloudEvents projector handler.
 * The method should return a CloudEvent or null to skip publishing.
 *
 * <p>The value is the event type suffix to match (e.g., "PlayerRegistered").
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface Publishes {
    /**
     * Event type suffix to match (e.g., "PlayerRegistered").
     */
    String value();
}
