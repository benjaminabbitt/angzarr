package dev.angzarr.client.annotations;

import com.google.protobuf.Message;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a method as a prepare handler for two-phase saga/PM protocol.
 * The method should return a list of Covers identifying destination aggregates.
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface Prepares {
    Class<? extends Message> value();
}
