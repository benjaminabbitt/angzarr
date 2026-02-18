package dev.angzarr.client;

import java.util.List;

/**
 * Describes what a component subscribes to or sends to.
 */
class TargetDesc {
    private final String domain;
    private final List<String> types;

    public TargetDesc(String domain, List<String> types) {
        this.domain = domain;
        this.types = types;
    }

    public String getDomain() {
        return domain;
    }

    public List<String> getTypes() {
        return types;
    }
}

/**
 * Describes a component for topology discovery.
 */
public class Descriptor {
    private final String name;
    private final String componentType;
    private final List<TargetDesc> inputs;

    public Descriptor(String name, String componentType, List<TargetDesc> inputs) {
        this.name = name;
        this.componentType = componentType;
        this.inputs = inputs;
    }

    public String getName() {
        return name;
    }

    public String getComponentType() {
        return componentType;
    }

    public List<TargetDesc> getInputs() {
        return inputs;
    }
}
