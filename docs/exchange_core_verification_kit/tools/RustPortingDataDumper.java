package exchange.core2.tests.util;

import exchange.core2.core.common.CoreSymbolSpecification;
import exchange.core2.core.common.cmd.OrderCommand;
import org.junit.jupiter.api.Test;

import java.io.FileWriter;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.BitSet;
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

        // Get intermediate data for UID verification
        List<BitSet> users2currencies = futures.getUsersAccounts().join();
        List<CoreSymbolSpecification> symbols = futures.getCoreSymbolSpecifications().join();

        TestOrdersGenerator.MultiSymbolGenResult result = futures.getGenResult().join();

        // There should be only 1 symbol for these datasets
        if (result.getGenResults().size() != 1) {
            throw new IllegalStateException("Expected 1 symbol for golden data");
        }

        CoreSymbolSpecification symbolSpec = symbols.get(0);
        TestOrdersGenerator.GenResult genResult = result.getGenResults().values().iterator().next();

        // 1. Export commands (original)
        try (FileWriter fw = new FileWriter(dir.resolve(filename + ".csv").toFile())) {
            fw.write("phase,command,order_id,symbol,price,size,action,order_type,uid\n");

            writeCommands(fw, "FILL", genResult.getCommandsFill());
            writeCommands(fw, "BENCHMARK", genResult.getCommandsBenchmark());
        }

        // 2. Export users2currencies mapping
        try (FileWriter fw = new FileWriter(dir.resolve(filename + "_users2currencies.csv").toFile())) {
            fw.write("uid,currencies\n");
            for (int uid = 0; uid < users2currencies.size(); uid++) {
                BitSet currencies = users2currencies.get(uid);
                StringBuilder sb = new StringBuilder();
                for (int c = currencies.nextSetBit(0); c >= 0; c = currencies.nextSetBit(c + 1)) {
                    if (sb.length() > 0)
                        sb.append(";");
                    sb.append(c);
                }
                fw.write(String.format("%d,%s\n", uid + 1, sb.toString())); // UID is 1-indexed
            }
        }

        // 3. Export uidsAvailableForSymbol array
        int symbolMessagesExpected = params.totalTransactionsNumber;
        int[] uidsForSymbol = UserCurrencyAccountsGenerator.createUserListForSymbol(
                users2currencies, symbolSpec, symbolMessagesExpected);

        try (FileWriter fw = new FileWriter(dir.resolve(filename + "_uids_for_symbol.csv").toFile())) {
            fw.write("index,uid\n");
            for (int i = 0; i < uidsForSymbol.length; i++) {
                fw.write(String.format("%d,%d\n", i, uidsForSymbol[i]));
            }
        }

        System.out.println("Written " + genResult.size() + " commands to " + filename + ".csv");
        System.out.println("Written " + users2currencies.size() + " user-currency mappings to " + filename
                + "_users2currencies.csv");
        System.out.println("Written " + uidsForSymbol.length + " UIDs to " + filename + "_uids_for_symbol.csv");
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
