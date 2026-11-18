import java.applet.Applet;
import java.io.File;
import java.net.URL;

/**
 * The applet seam: the original code resolves every asset through
 * Applet.getCodeBase(), exactly as the 2005 applet tag's codebase
 * attribute did. A test subclass overrides it to a file: URL so the
 * real loading paths run against the shipped assets - the original
 * sources are never touched.
 */
public class TestApplet extends Applet {
   private final URL base;

   TestApplet(String dir) throws Exception {
      String u = new File(dir).toURI().toURL().toExternalForm();
      if (!u.endsWith("/")) {
         // without the trailing slash, relative URLs would resolve as
         // siblings of the directory instead of inside it
         u = u + "/";
      }
      base = new URL(u);
   }

   public URL getCodeBase() {
      return base;
   }

   public URL getDocumentBase() {
      return base;
   }
}
