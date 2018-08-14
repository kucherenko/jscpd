/*
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not
 * distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */
import std.stdio;

import std.array;
import std.experimental.logger;
import std.file;
import std.format;
import std.process;
import std.string;

import glib.FileUtils;
import glib.Util;

import gtk.Main;
import gtk.Version;
import gtk.MessageDialog;

import gx.i18n.l10n;
import gx.gtk.util;

import gx.tilix.application;
import gx.tilix.cmdparams;
import gx.tilix.constants;

int main(string[] args) {
    static if (USE_FILE_LOGGING) {
        sharedLog = new FileLogger("/tmp/tilix.log");
    }

    bool newProcess = false;
    string cwd = Util.getCurrentDir();
    string pwd;
    string de;
    trace("CWD = " ~ cwd);
    try {
        pwd = environment["PWD"];
        de = environment["XDG_CURRENT_DESKTOP"];
        trace("PWD = " ~ pwd);
    } catch (Exception e) {
        trace("No PWD environment variable found");
    }
    try {
        environment.remove("WINDOWID");
    } catch (Exception e) {
        error("Unexpected error occurred", e);
    }

    string uhd = Util.getHomeDir();
    trace("UHD = " ~ uhd);

    //Debug args
    foreach(i, arg; args) {
        tracef("args[%d]=%s", i, arg);
    }

    // Look for execute command and convert it into a normal -e
    // We do this because this switch means take everything after
    // the switch as a command which GApplication options cannot handle
    // without a callback which D doesn't expose at this time.
    foreach(i, arg; args) {
        if (arg == "-x" || arg == "-e") {
            string executeCommand;
            // Are we dealing with a single command that either
            // has no spaces or been escaped by the user or a string
            // of multiple commands
            if (args.length == i + 2) {
                trace("Single command");
                executeCommand = args[i + 1];
            } else {
                for(size_t j=i+1; j<args.length; j++) {
                    if (j > i + 1) {
                        executeCommand ~= " ";
                    }
                    if (args[j].indexOf(" ") > 0) {
                        executeCommand ~= "\"" ~ replace(args[j], "\"", "\\\"") ~ "\"";
                    } else {
                        executeCommand ~= args[j];
                    }
                }
            }
            trace("Execute Command: " ~ executeCommand);
            args = args[0..i];
            if (arg == "-x") {
                args ~= "-e";
            } else {
                args ~= arg;
            }
            args ~= executeCommand;
            break;
        }
    }

    //textdomain
    textdomain(TILIX_DOMAIN);
    // Init GTK early so localization is available
    // Note used to pass empty args but was interfering with GTK default args
    Main.init(args);

    trace(format("Starting tilix with %d arguments...", args.length));
    foreach(i, arg; args) {
        trace(format("arg[%d] = %s",i, arg));
        // Workaround issue with Unity and older Gnome Shell when DBusActivatable sometimes CWD is set to /, see #285
        if (arg == "--gapplication-service" && pwd == uhd && cwd == "/") {
            info("Detecting DBusActivatable with improper directory, correcting by setting CWD to PWD");
            infof("CWD = %s", cwd);
            infof("PWD = %s", pwd);
            cwd = pwd;
            FileUtils.chdir(cwd);
        } else if (arg == "--new-process") {
            newProcess = true;
        } else if (arg == "-v" || arg == "--version") {
            outputVersions();
            return 0;
        }
    }
    //append TILIX_ID to args if present
    try {
        string terminalUUID = environment["TILIX_ID"];
        trace("Inserting terminal UUID " ~ terminalUUID);
        args ~= ("--" ~ CMD_TERMINAL_UUID ~ "=" ~ terminalUUID);
    }
    catch (Exception e) {
        trace("No tilix UUID found");
    }

    //Version checking cribbed from grestful, thanks!
    string gtkError = Version.checkVersion(GTK_VERSION_MAJOR, GTK_VERSION_MINOR, GTK_VERSION_PATCH);
    if (gtkError !is null) {
        MessageDialog dialog = new MessageDialog(null, DialogFlags.MODAL, MessageType.ERROR, ButtonsType.OK,
                format(_("Your GTK version is too old, you need at least GTK %d.%d.%d!"), GTK_VERSION_MAJOR, GTK_VERSION_MINOR, GTK_VERSION_PATCH), null);

        dialog.setDefaultResponse(ResponseType.OK);

        dialog.run();
        return 1;
    }

    trace("Creating app");
    auto tilixApp = new Tilix(newProcess);
    int result;
    try {
        trace("Running application...");
        result = tilixApp.run(args);
        trace("App completed...");
    }
    catch (Exception e) {
        error(_("Unexpected exception occurred"));
        error(_("Error: ") ~ e.msg);
    }
    return result;
}

private:
    void outputVersions() {
        import gx.gtk.vte: getVTEVersion, checkVTEFeature, TerminalFeature;
        import gtk.Version: Version;

        writeln(_("Versions"));
        writeln("\t" ~ format(_("Tilix version: %s"), APPLICATION_VERSION));
        writeln("\t" ~ format(_("VTE version: %s"), getVTEVersion()));
        writeln("\t" ~ format(_("GTK Version: %d.%d.%d") ~ "\n", Version.getMajorVersion(), Version.getMinorVersion(), Version.getMicroVersion()));
        writeln(_("Tilix Special Features"));
        writeln("\t" ~ format(_("Notifications enabled=%b"), checkVTEFeature(TerminalFeature.EVENT_NOTIFICATION)));
        writeln("\t" ~ format(_("Triggers enabled=%b"), checkVTEFeature(TerminalFeature.EVENT_SCREEN_CHANGED)));
        writeln("\t" ~ format(_("Badges enabled=%b"), checkVTEFeature(TerminalFeature.DISABLE_BACKGROUND_DRAW)));
    }