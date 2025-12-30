package exchange.core2.tests.util;

import exchange.core2.core.common.cmd.OrderCommand;
import org.junit.jupiter.api.Test;

import java.io.FileWriter;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

public class RustPortingDataDumper {

    public static void main(String[] args) throws Exception {
        new RustPortingDataDumper().dumpGoldenData();
    }

    @Test
    public void dumpGoldenData() throws Exception {
        Path outputDir = Paths.get("docs/exchange_core_verification_kit/golden_data");
        Files.createDirectories(outputDir);

        dumpDataset(outputDir, "golden_single_pair_margin", TestDataParameters.singlePairMarginBuilder());
        dumpDataset(outputDir, "golden_single_pair_exchange", TestDataParameters.singlePairExchangeBuilder());
    }

    private void dumpDataset(Path dir, String filename, TestDataParameters.TestDataParametersBuilder builder)
            throws Exception {
        // Reduced size for golden data
        // 1000 transactions is enough for logic verification
        TestDataParameters params = builder
                .totalTransactionsNumber(1000)
                .targetOrderBookOrdersTotal(100)
                .numAccounts(100)
                .build();

        System.out.println("Generating data for " + filename + "...");
        // Fixed seed = 1 to ensure deterministic output
        ExchangeTestContainer.TestDataFutures futures = ExchangeTestContainer.prepareTestDataAsync(params, 1);
        TestOrdersGenerator.MultiSymbolGenResult result = futures.getGenResult().join();

        // There should be only 1 symbol for these datasets
        if (result.getGenResults().size() != 1) {
            throw new IllegalStateException("Expected 1 symbol for golden data");
        }

        TestOrdersGenerator.GenResult genResult = result.getGenResults().values().iterator().next();

        try (FileWriter fw = new FileWriter(dir.resolve(filename + ".csv").toFile())) {
            fw.write("phase,command,order_id,symbol,price,size,action,order_type,uid\n");

            writeCommands(fw, "FILL", genResult.getCommandsFill());
            writeCommands(fw, "BENCHMARK", genResult.getCommandsBenchmark());
        }

        System.out.println("Written " + genResult.size() + " commands to " + filename + ".csv");
    }

    private void writeCommands(FileWriter fw, String phase, List<OrderCommand> commands) throws IOException {
        for (OrderCommand cmd : commands) {
            // CSV format: phase, command, orderId, symbol, price, size, action, orderType,
            // uid
            fw.write(String.format("%s,%s,%d,%d,%d,%d,%s,%s,%d\n",
                    phase,
                    cmd.command,
                    cmd.orderId,
                    cmd.symbol,
                    cmd.price,
                    cmd.size,
                    cmd.action,
                    cmd.orderType,
                    cmd.uid));
        }
    }
}
