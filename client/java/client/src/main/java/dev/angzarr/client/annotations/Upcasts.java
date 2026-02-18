package dev.angzarr.client.annotations;

import com.google.protobuf.Message;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a method as an upcaster for event version transformation.
 * The method should return the new event version.
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface Upcasts {
    Class<? extends Message> from();
    Class<? extends Message> to();
}
