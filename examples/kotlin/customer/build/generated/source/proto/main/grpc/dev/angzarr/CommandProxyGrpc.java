package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 * <pre>
 * Proxy service - send command and receive resulting events
 * </pre>
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class CommandProxyGrpc {

  private CommandProxyGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.CommandProxy";

  // Static method descriptors that strictly reflect the proto.
  private static volatile io.grpc.MethodDescriptor<dev.angzarr.CommandBook,
      dev.angzarr.EventBook> getExecuteMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "Execute",
      requestType = dev.angzarr.CommandBook.class,
      responseType = dev.angzarr.EventBook.class,
      methodType = io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
  public static io.grpc.MethodDescriptor<dev.angzarr.CommandBook,
      dev.angzarr.EventBook> getExecuteMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.CommandBook, dev.angzarr.EventBook> getExecuteMethod;
    if ((getExecuteMethod = CommandProxyGrpc.getExecuteMethod) == null) {
      synchronized (CommandProxyGrpc.class) {
        if ((getExecuteMethod = CommandProxyGrpc.getExecuteMethod) == null) {
          CommandProxyGrpc.getExecuteMethod = getExecuteMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.CommandBook, dev.angzarr.EventBook>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Execute"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.CommandBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setSchemaDescriptor(new CommandProxyMethodDescriptorSupplier("Execute"))
              .build();
        }
      }
    }
    return getExecuteMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static CommandProxyStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<CommandProxyStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<CommandProxyStub>() {
        @java.lang.Override
        public CommandProxyStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new CommandProxyStub(channel, callOptions);
        }
      };
    return CommandProxyStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static CommandProxyBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<CommandProxyBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<CommandProxyBlockingStub>() {
        @java.lang.Override
        public CommandProxyBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new CommandProxyBlockingStub(channel, callOptions);
        }
      };
    return CommandProxyBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static CommandProxyFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<CommandProxyFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<CommandProxyFutureStub>() {
        @java.lang.Override
        public CommandProxyFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new CommandProxyFutureStub(channel, callOptions);
        }
      };
    return CommandProxyFutureStub.newStub(factory, channel);
  }

  /**
   * <pre>
   * Proxy service - send command and receive resulting events
   * </pre>
   */
  public interface AsyncService {

    /**
     * <pre>
     * Send command, receive stream of resulting events
     * Automatically generates correlation_id if not provided
     * Streams events back to client as they occur
     * </pre>
     */
    default void execute(dev.angzarr.CommandBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getExecuteMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service CommandProxy.
   * <pre>
   * Proxy service - send command and receive resulting events
   * </pre>
   */
  public static abstract class CommandProxyImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return CommandProxyGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service CommandProxy.
   * <pre>
   * Proxy service - send command and receive resulting events
   * </pre>
   */
  public static final class CommandProxyStub
      extends io.grpc.stub.AbstractAsyncStub<CommandProxyStub> {
    private CommandProxyStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected CommandProxyStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new CommandProxyStub(channel, callOptions);
    }

    /**
     * <pre>
     * Send command, receive stream of resulting events
     * Automatically generates correlation_id if not provided
     * Streams events back to client as they occur
     * </pre>
     */
    public void execute(dev.angzarr.CommandBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ClientCalls.asyncServerStreamingCall(
          getChannel().newCall(getExecuteMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service CommandProxy.
   * <pre>
   * Proxy service - send command and receive resulting events
   * </pre>
   */
  public static final class CommandProxyBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<CommandProxyBlockingStub> {
    private CommandProxyBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected CommandProxyBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new CommandProxyBlockingStub(channel, callOptions);
    }

    /**
     * <pre>
     * Send command, receive stream of resulting events
     * Automatically generates correlation_id if not provided
     * Streams events back to client as they occur
     * </pre>
     */
    public java.util.Iterator<dev.angzarr.EventBook> execute(
        dev.angzarr.CommandBook request) {
      return io.grpc.stub.ClientCalls.blockingServerStreamingCall(
          getChannel(), getExecuteMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service CommandProxy.
   * <pre>
   * Proxy service - send command and receive resulting events
   * </pre>
   */
  public static final class CommandProxyFutureStub
      extends io.grpc.stub.AbstractFutureStub<CommandProxyFutureStub> {
    private CommandProxyFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected CommandProxyFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new CommandProxyFutureStub(channel, callOptions);
    }
  }

  private static final int METHODID_EXECUTE = 0;

  private static final class MethodHandlers<Req, Resp> implements
      io.grpc.stub.ServerCalls.UnaryMethod<Req, Resp>,
      io.grpc.stub.ServerCalls.ServerStreamingMethod<Req, Resp>,
      io.grpc.stub.ServerCalls.ClientStreamingMethod<Req, Resp>,
      io.grpc.stub.ServerCalls.BidiStreamingMethod<Req, Resp> {
    private final AsyncService serviceImpl;
    private final int methodId;

    MethodHandlers(AsyncService serviceImpl, int methodId) {
      this.serviceImpl = serviceImpl;
      this.methodId = methodId;
    }

    @java.lang.Override
    @java.lang.SuppressWarnings("unchecked")
    public void invoke(Req request, io.grpc.stub.StreamObserver<Resp> responseObserver) {
      switch (methodId) {
        case METHODID_EXECUTE:
          serviceImpl.execute((dev.angzarr.CommandBook) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.EventBook>) responseObserver);
          break;
        default:
          throw new AssertionError();
      }
    }

    @java.lang.Override
    @java.lang.SuppressWarnings("unchecked")
    public io.grpc.stub.StreamObserver<Req> invoke(
        io.grpc.stub.StreamObserver<Resp> responseObserver) {
      switch (methodId) {
        default:
          throw new AssertionError();
      }
    }
  }

  public static final io.grpc.ServerServiceDefinition bindService(AsyncService service) {
    return io.grpc.ServerServiceDefinition.builder(getServiceDescriptor())
        .addMethod(
          getExecuteMethod(),
          io.grpc.stub.ServerCalls.asyncServerStreamingCall(
            new MethodHandlers<
              dev.angzarr.CommandBook,
              dev.angzarr.EventBook>(
                service, METHODID_EXECUTE)))
        .build();
  }

  private static abstract class CommandProxyBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    CommandProxyBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("CommandProxy");
    }
  }

  private static final class CommandProxyFileDescriptorSupplier
      extends CommandProxyBaseDescriptorSupplier {
    CommandProxyFileDescriptorSupplier() {}
  }

  private static final class CommandProxyMethodDescriptorSupplier
      extends CommandProxyBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    CommandProxyMethodDescriptorSupplier(java.lang.String methodName) {
      this.methodName = methodName;
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.MethodDescriptor getMethodDescriptor() {
      return getServiceDescriptor().findMethodByName(methodName);
    }
  }

  private static volatile io.grpc.ServiceDescriptor serviceDescriptor;

  public static io.grpc.ServiceDescriptor getServiceDescriptor() {
    io.grpc.ServiceDescriptor result = serviceDescriptor;
    if (result == null) {
      synchronized (CommandProxyGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new CommandProxyFileDescriptorSupplier())
              .addMethod(getExecuteMethod())
              .build();
        }
      }
    }
    return result;
  }
}
