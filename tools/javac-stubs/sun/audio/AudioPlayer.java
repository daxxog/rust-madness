package sun.audio;

import java.io.InputStream;

/**
 * Compile-time stub of the JDK 1.x internal {@code sun.audio} API that the
 * original applet uses ({@code AudioPlayer.player.start/stop} for the
 * built-in .XM music playback). Modern JDKs ship no such classes, so the
 * decompiled trees can only round-trip compile with this stub present.
 *<p>
 * For the Rust port this is irrelevant except to know that playback used
 * sun.audio with μ-law converted samples (see UlawUtils) fed to
 * AudioPlayer.player as a raw InputStream.
 */
public class AudioPlayer {
    public static final AudioPlayer player = new AudioPlayer();

    public void start(InputStream stream) { }

    public void stop(InputStream stream) { }
}
