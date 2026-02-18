package dev.angzarr.client.annotations;

import com.google.protobuf.Message;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a method as an event applier for the specified event type.
 * The method should mutate state in place.
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface Applies {
    Class<? extends Message> value();
}
