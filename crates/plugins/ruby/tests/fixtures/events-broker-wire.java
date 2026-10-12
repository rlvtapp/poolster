// Test-only malformed Kafka records. Apache Kafka clients 3.9.1.
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Properties;
import java.util.concurrent.TimeUnit;
import org.apache.kafka.clients.producer.KafkaProducer;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.common.header.Header;
import org.apache.kafka.common.header.internals.RecordHeader;
import org.apache.kafka.common.serialization.ByteArraySerializer;

class WireInjector {
  static byte[] bytes(String value) { return value.getBytes(StandardCharsets.UTF_8); }
  public static void main(String[] args) throws Exception {
    Properties properties = new Properties();
    properties.put("bootstrap.servers", "127.0.0.1:29092");
    properties.put("client.id", "poolster-wire-injector");
    properties.put("acks", "all");
    properties.put("delivery.timeout.ms", "30000");
    properties.put("request.timeout.ms", "10000");
    byte[] valid = bytes("{\"id\":\"wire\",\"quantity\":1}");
    try (KafkaProducer<byte[], byte[]> producer = new KafkaProducer<>(properties, new ByteArraySerializer(), new ByteArraySerializer())) {
      for (int index = 0; index < 8; index++) {
        byte[] key = bytes("key");
        byte[] value = valid;
        List<Header> headers = List.of(new RecordHeader("trace", bytes("trace")));
        switch (index) {
          case 0 -> value = new byte[]{(byte)255};
          case 1 -> key = new byte[]{(byte)255};
          case 2 -> headers = List.of(new RecordHeader("trace", new byte[]{(byte)255}));
          case 3 -> headers = List.of(new RecordHeader("trace", null));
          case 4 -> value = null;
          case 5 -> value = bytes("{\"id\":\"x\",\"id\":\"y\",\"quantity\":1}");
          case 6 -> headers = List.of(new RecordHeader("trace", bytes("a")), new RecordHeader("trace", bytes("b")));
          case 7 -> value = bytes("{\"id\":\"wire\",\"quantity\":0}");
        }
        String topic = args[0] + "-bad-" + index;
        producer.send(new ProducerRecord<>(topic, 0, null, key, value, headers)).get(30, TimeUnit.SECONDS);
        producer.send(new ProducerRecord<>(topic, 0, null, bytes("key"), valid, List.of(new RecordHeader("trace", bytes("trace"))))).get(30, TimeUnit.SECONDS);
      }
      producer.flush();
    }
  }
}
