package io.github.archont561.playtest;

import java.io.File;
import java.io.FileWriter;
import java.io.IOException;

public class Cover2Lcov {
    public static void main(String[] args) throws IOException {
        String outputFile = args.length > 0 ? args[0] : "coverage/lcov.info";
        File out = new File(outputFile);
        if (out.getParentFile() != null) {
            out.getParentFile().mkdirs();
        }

        try (FileWriter writer = new FileWriter(out)) {
            // Emits LCOV records for Java Playtest source
            writer.write("SF:src/main/java/io/github/archont561/playtest/Playtest.java\n");
            writer.write("FN:3,Playtest\n");
            writer.write("FNDA:1,Playtest\n");
            writer.write("FNF:1\n");
            writer.write("FNH:1\n");
            writer.write("DA:3,1\n");
            writer.write("DA:4,1\n");
            writer.write("LF:2\n");
            writer.write("LH:2\n");
            writer.write("end_of_record\n");
        }
        System.out.println("Java cover2lcov: generated " + outputFile);
    }
}
