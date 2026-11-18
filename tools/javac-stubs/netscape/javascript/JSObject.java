package netscape.javascript;

import java.applet.Applet;

/**
 * Compile-time stub of the Java-plugin-era liveconnect class
 * {@code netscape.javascript.JSObject}, which the applet uses in
 * GameSparker's testlocation()/readcookie()/savecookie() to talk to
 * JavaScript cookies on the host page.
 *
 * The JDK's own jdk.jsobject module ships an ABSTRACT JSObject without the
 * static getWindow(Applet) factory (which lives in the browser plugin's
 * subclass), so this stub replaces it. The round-trip javac invocation
 * passes --limit-modules to keep jdk.jsobject unobservable and avoid a
 * package/module name conflict.
 */
public class JSObject {
    public static JSObject getWindow(Applet applet) {
        return null;
    }

    public Object eval(String s) {
        return null;
    }

    public Object getMember(String s) {
        return null;
    }
}
