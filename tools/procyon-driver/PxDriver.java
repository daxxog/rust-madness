/*
 * Minimal replacement for the procyon CLI (DecompilerDriver), which the
 * Maven Central `procyon-compilertools` artifact does not ship. Uses the
 * library API directly: one file per top-level class in the input jar,
 * written to the output directory.
 *
 * Built in the tools image:  javac -cp procyon.jar PxDriver.java
 * Run:                       java -cp procyon.jar:tools PxDriver <input.jar> <outdir> [logfile]
 */
import com.strobel.assembler.metadata.JarTypeLoader;
import com.strobel.decompiler.Decompiler;
import com.strobel.decompiler.DecompilerSettings;
import com.strobel.decompiler.PlainTextOutput;

import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.io.PrintWriter;
import java.util.Enumeration;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;

public final class PxDriver {

    public static void main(final String[] args) {
        if (args.length < 2) {
            System.err.println("usage: PxDriver <input.jar> <outdir> [logfile]");
            System.exit(2);
        }
        final JarFile jar;
        try {
            jar = new JarFile(args[0]);
        } catch (IOException e) {
            System.err.println("cannot open jar: " + e);
            System.exit(2);
            return;
        }
        final File outdir = new File(args[1]);
        outdir.mkdirs();
        final boolean logAll = args.length > 2;
        final PrintWriter errLog;
        try {
            errLog = logAll ? new PrintWriter(new FileWriter(args[2])) : new PrintWriter(new FileWriter(java.io.FileDescriptor.err));
        } catch (IOException e) {
            System.err.println("cannot open log: " + e);
            System.exit(2);
            return;
        }

        final DecompilerSettings settings = DecompilerSettings.javaDefaults();
        settings.setTypeLoader(new JarTypeLoader(jar));
        settings.setExcludeNestedTypes(false);

        int written = 0;
        int failed = 0;
        for (final Enumeration<JarEntry> e = jar.entries(); e.hasMoreElements(); ) {
            final String path = e.nextElement().getName();
            if (!path.endsWith(".class")) {
                continue;
            }
            final String cls = path.substring(0, path.length() - ".class".length()).replace('/', '.');
            final File target = new File(outdir, cls.replace('.', '.') + ".java");
            try {
                final FileWriter w = new FileWriter(target);
                try {
                    Decompiler.decompile(cls, new PlainTextOutput(w), settings);
                } finally {
                    w.close();
                }
                written++;
            } catch (Throwable t) {
                failed++;
                errLog.println("FAILED decompiling " + cls + ": " + t);
            }
        }
        errLog.flush();
        errLog.close();
        System.out.println("PxDriver: written=" + written + " failed=" + failed);
    }
}
