/*   **********************************************************************  **
 **   Copyright notice                                                       **
 **                                                                          **
 **   (c) 2005-2009 RSSOwl Development Team                                  **
 **   http://www.rssowl.org/                                                 **
 **                                                                          **
 **   All rights reserved                                                    **
 **                                                                          **
 **   This program and the accompanying materials are made available under   **
 **   the terms of the Eclipse Public License v1.0 which accompanies this    **
 **   distribution, and is available at:                                     **
 **   http://www.rssowl.org/legal/epl-v10.html                               **
 **                                                                          **
 **   A copy is found in the file epl-v10.html and important notices to the  **
 **   license from the team is found in the textfile LICENSE.txt distributed **
 **   in this package.                                                       **
 **                                                                          **
 **   This copyright notice MUST APPEAR in all copies of the file!           **
 **                                                                          **
 **   Contributors:                                                          **
 **     RSSOwl Development Team - initial API and implementation             **
 **                                                                          **
 **  **********************************************************************  */

package org.rssowl.core;

import org.eclipse.core.runtime.Assert;
import org.rssowl.core.connection.IConnectionService;
import org.rssowl.core.connection.ICredentialsProvider;
import org.rssowl.core.connection.IProtocolHandler;
import org.rssowl.core.internal.InternalOwl;
import org.rssowl.core.interpreter.IElementHandler;
import org.rssowl.core.interpreter.IFormatInterpreter;
import org.rssowl.core.interpreter.IInterpreterService;
import org.rssowl.core.interpreter.INamespaceHandler;
import org.rssowl.core.interpreter.IXMLParser;
import org.rssowl.core.persist.IModelFactory;
import org.rssowl.core.persist.dao.DAOService;
import org.rssowl.core.persist.dao.DynamicDAO;
import org.rssowl.core.persist.pref.IPreferenceScope;
import org.rssowl.core.persist.pref.IPreferencesInitializer;
import org.rssowl.core.persist.service.IModelSearch;
import org.rssowl.core.persist.service.IPersistenceService;
import org.rssowl.core.persist.service.IPreferenceService;
import org.rssowl.core.util.LongOperationMonitor;

/**
 * The <code>Owl</code> class is the main facade to all API in RSSOwl. It offers
 * access to services, such as for persistence, search, model and interpreter.
 * Note that in some cases directly using the <code>DynamicDAO</code> class
 * might be shorter.
 *
 * @author bpasero
 * @see DynamicDAO
 */
public final class Owl {

  /**
   * Gives extra information on the state to the
   * {@link Owl#startup(LongOperationMonitor)} call
   */
  public enum StartLevel {
    NOT_STARTED, STARTING, DB_OPENED, SEARCH_INDEX_OPENED, STARTED;
  }

  /**
   * Returns the {@link StartLevel} as reached from a call to the
   * {@link #startup(LongOperationMonitor)} sequence.
   *
   * @return the {@link StartLevel} from the
   * {@link #startup(LongOperationMonitor)} sequence.
   */
  public static StartLevel getStartLevel() {
    return InternalOwl.getDefault().getStartLevel();
  }

  /**
   * <p>
   * Get the Implementation of <code>IApplicationService</code> that contains
   * special Methods which are used through the Application and access the
   * persistence layer. The implementation is looked up using the
   * "org.rssowl.core.model.ApplicationService" Extension Point.
   * </p>
   * Subclasses may override to provide their own implementation.
   *
   * @return Returns the Implementation of <code>IApplicationService</code> that
   * contains special Methods which are used through the Application and access
   * the persistence layer.
   */
  public static IApplicationService getApplicationService() {
    Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization"); //$NON-NLS-1$
    return InternalOwl.getDefault().getApplicationService();
  }

  /**
   * <p>
   * Provides access to the scoped preferences service in RSSOwl. There is three
   * levels of preferences: Default, Global and Entity. Any preference that is
   * not set at the one scope will be looked up in the parent scope until the
   * Default scope is reached. This allows to easily override the preferences
   * for all entities without having to define the preferences per entity.
   * </p>
   * <p>
   * You can define default preferences by using the PreferencesInitializer
   * extension point provided by this plugin.
   * </p>
   *
   * @return Returns the IPreferenceService that provides access to the scoped
   * preferences system in RSSOwl.
   * @see IPreferenceScope
   * @see IPreferencesInitializer
   */
  public static IPreferenceService getPreferenceService() {
    Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization"); //$NON-NLS-1$
    return InternalOwl.getDefault().getPreferenceService();
  }

  /**
   * Provides access to ther persistence layer of RSSOwl. This layer is
   * contributable via the PersistenceService extension point provided by this
   * plugin. The work that is done by the layer includes:
   * <ul>
   * <li>Controlling the lifecycle of the persistence layer</li>
   * <li>Providing the DAOService that contains DAOs for each persistable entity
   * </li>
   * <li>Providing the model search to perform full-text searching</li>
   * </ul>
   *
   * @return Returns the service responsible for all persistence related tasks.
   * @see DAOService
   * @see IModelSearch
   */
  public static IPersistenceService getPersistenceService() {
    Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization"); //$NON-NLS-1$
    return InternalOwl.getDefault().getPersistenceService();
  }

  /**
   * Provides access to the connection service of RSSOwl. This service provides
   * API to load data from the internet (e.g. loading the contents of a feed).
   * It is also the central place to ask for credentials if a resource requires
   * authentication. Several extension points allow to customize the behavor of
   * this service, including the ability to register
   * <code>IProtocolHandler</code> to define the lookup process on per protocol
   * basis or contributing <code>ICredentialsProvider</code> to define how
   * credentials should be stored and retrieved.
   *
   * @return Returns the service responsible for all connection related tasks.
   * @see IProtocolHandler
   * @see ICredentialsProvider
   */
  public static IConnectionService getConnectionService() {
    Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization"); //$NON-NLS-1$
    return InternalOwl.getDefault().getConnectionService();
  }

  /**
   * Provides access to the interpreter service of RSSOwl. This service provides
   * API to convert a stream of data into a model representation. In the common
   * case of a XML stream this involves using a XML-Parser and creating the
   * model out of the content. Various extension points allow to customize the
   * behavor of the interpreter:
   * <ul>
   * <li>Contribute a new format interpreter using the FormatInterpreter
   * extension point. This allows to display any XML in RSSOwl as Feed.</li>
   * <li>Contribute a new namespace handler using the NamespaceHandler extension
   * point. This allows to properly handle any new namespace in RSSOwl.</li>
   * <li>Contribute a new element handler using the ElementHandler extension
   * point. This makes RSSOwl understand new elements or even attributes.</li>
   * <li>Contribute a new xml parser using the XMLParser extension point if you
   * are not happy with the default one.</li>
   * </ul>
   *
   * @return Returns the service responsible for interpreting a resource.
   * @see IFormatInterpreter
   * @see IElementHandler
   * @see INamespaceHandler
   * @see IXMLParser
   */
  public static IInterpreterService getInterpreter() {
    Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization"); //$NON-NLS-1$
    return InternalOwl.getDefault().getInterpreter();
  }

  /**
   * Triggers the startup sequence of the Owl core. Will return immediately if
   * the core has already been started.
   *
   * @param monitor A progress monitor to report progress on long running
   * operations (e.g. migration).
   */
  public static void startup(LongOperationMonitor monitor) {
    if (!InternalOwl.getDefault().isStarted())
      InternalOwl.getDefault().startup(monitor, false, false);
  }

  /**
   * @return <code>true</code> if {@link Owl#startup(LongOperationMonitor)} has
   * been called already and <code>false</code> otherwise.
   */
  public static boolean isStarted() {
    return InternalOwl.getDefault().isStarted();
  }

  /**
   * @param emergency If set to <code>TRUE</code>, this method is called from a
   * shutdown hook that got triggered from a non-normal shutdown (e.g. System
   * Shutdown).
   */
  public static void shutdown(boolean emergency) {
    InternalOwl.getDefault().shutdown(emergency);
  }

  /**
   * @return <code>true</code> if {@link Owl#shutdown(boolean)} has been called
   * already and <code>false</code> otherwise.
   */
  public static boolean isShuttingDown() {
    return InternalOwl.getDefault().isShuttingDown();
  }
}
