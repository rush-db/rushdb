// Parses each line of standard input as a Gremlin query with the official
// TinkerPop ANTLR grammar (`queryList`) and reports every line that does not
// parse. Exit status 1 when any line fails. Built by
// scripts/validate-gremlin-grammar.sh.

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;

import org.antlr.v4.runtime.BaseErrorListener;
import org.antlr.v4.runtime.CharStreams;
import org.antlr.v4.runtime.CommonTokenStream;
import org.antlr.v4.runtime.RecognitionException;
import org.antlr.v4.runtime.Recognizer;

public final class ValidateGremlin {
    private static final class Collect extends BaseErrorListener {
        String error;

        @Override
        public void syntaxError(Recognizer<?, ?> recognizer, Object symbol, int line, int column,
                String message, RecognitionException cause) {
            if (error == null) {
                error = column + ": " + message;
            }
        }
    }

    public static void main(String[] args) throws Exception {
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        int total = 0;
        int failed = 0;
        String line;
        while ((line = input.readLine()) != null) {
            if (line.isBlank()) {
                continue;
            }
            total++;
            Collect errors = new Collect();
            GremlinLexer lexer = new GremlinLexer(CharStreams.fromString(line));
            lexer.removeErrorListeners();
            lexer.addErrorListener(errors);
            GremlinParser parser = new GremlinParser(new CommonTokenStream(lexer));
            parser.removeErrorListeners();
            parser.addErrorListener(errors);
            parser.queryList();
            if (errors.error != null) {
                failed++;
                System.out.println("INVALID " + line + "  -- " + errors.error);
            }
        }
        System.out.println(total + " traversals, " + failed + " rejected by the TinkerPop grammar");
        System.exit(failed == 0 ? 0 : 1);
    }
}
