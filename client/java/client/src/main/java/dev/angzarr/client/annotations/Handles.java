package dev.angzarr.client.annotations;

import com.google.protobuf.Message;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a method as a command handler for the specified command type.
 * The method should return an event or collection of events.
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface Handles {
    Class<? extends Message> value();
}
