package dev.angzarr.client.annotations;

import com.google.protobuf.Message;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a method as an event handler for sagas or process managers.
 * The method should return a command or collection of commands.
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface ReactsTo {
    Class<? extends Message> value();
    String inputDomain() default "";
    String outputDomain() default "";
}
