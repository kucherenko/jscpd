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

package org.rssowl.core

import org.eclipse.core.runtime.Assert
import org.rssowl.core.connection.IConnectionService
import org.rssowl.core.connection.ICredentialsProvider
import org.rssowl.core.connection.IProtocolHandler
import org.rssowl.core.internal.InternalOwl
import org.rssowl.core.interpreter.IElementHandler
import org.rssowl.core.interpreter.IFormatInterpreter
import org.rssowl.core.interpreter.IInterpreterService
import org.rssowl.core.interpreter.INamespaceHandler
import org.rssowl.core.interpreter.IXMLParser
import org.rssowl.core.persist.IModelFactory
import org.rssowl.core.persist.dao.DAOService
import org.rssowl.core.persist.dao.DynamicDAO
import org.rssowl.core.persist.pref.IPreferenceScope
import org.rssowl.core.persist.pref.IPreferencesInitializer
import org.rssowl.core.persist.service.IModelSearch
import org.rssowl.core.persist.service.IPersistenceService
import org.rssowl.core.persist.service.IPreferenceService
import org.rssowl.core.util.LongOperationMonitor

/**
 * The `Owl` class is the main facade to all API in RSSOwl. It offers
 * access to services, such as for persistence, search, model and interpreter.
 * Note that in some cases directly using the `DynamicDAO` class
 * might be shorter.
 *
 * @author bpasero
 * @see DynamicDAO
 */
object Owl {

    /**
     * Returns the [StartLevel] as reached from a call to the
     * [.startup] sequence.
     *
     * @return the [StartLevel] from the
     * [.startup] sequence.
     */
    val startLevel: StartLevel
        get() = InternalOwl.getDefault().getStartLevel()

    /**
     *
     *
     * Get the Implementation of `IApplicationService` that contains
     * special Methods which are used through the Application and access the
     * persistence layer. The implementation is looked up using the
     * "org.rssowl.core.model.ApplicationService" Extension Point.
     *
     * Subclasses may override to provide their own implementation.
     *
     * @return Returns the Implementation of `IApplicationService` that
     * contains special Methods which are used through the Application and access
     * the persistence layer.
     */
    val applicationService: IApplicationService
        get() {
            Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization")
            return InternalOwl.getDefault().getApplicationService()
        }

    /**
     *
     *
     * Provides access to the scoped preferences service in RSSOwl. There is three
     * levels of preferences: Default, Global and Entity. Any preference that is
     * not set at the one scope will be looked up in the parent scope until the
     * Default scope is reached. This allows to easily override the preferences
     * for all entities without having to define the preferences per entity.
     *
     *
     *
     * You can define default preferences by using the PreferencesInitializer
     * extension point provided by this plugin.
     *
     *
     * @return Returns the IPreferenceService that provides access to the scoped
     * preferences system in RSSOwl.
     * @see IPreferenceScope
     *
     * @see IPreferencesInitializer
     */
    val preferenceService: IPreferenceService
        get() {
            Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization")
            return InternalOwl.getDefault().getPreferenceService()
        }

    /**
     * Provides access to ther persistence layer of RSSOwl. This layer is
     * contributable via the PersistenceService extension point provided by this
     * plugin. The work that is done by the layer includes:
     *
     *  * Controlling the lifecycle of the persistence layer
     *  * Providing the DAOService that contains DAOs for each persistable entity
     *
     *  * Providing the model search to perform full-text searching
     *
     *
     * @return Returns the service responsible for all persistence related tasks.
     * @see DAOService
     *
     * @see IModelSearch
     */
    val persistenceService: IPersistenceService
        get() {
            Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization")
            return InternalOwl.getDefault().getPersistenceService()
        }

    /**
     * Provides access to the connection service of RSSOwl. This service provides
     * API to load data from the internet (e.g. loading the contents of a feed).
     * It is also the central place to ask for credentials if a resource requires
     * authentication. Several extension points allow to customize the behavor of
     * this service, including the ability to register
     * `IProtocolHandler` to define the lookup process on per protocol
     * basis or contributing `ICredentialsProvider` to define how
     * credentials should be stored and retrieved.
     *
     * @return Returns the service responsible for all connection related tasks.
     * @see IProtocolHandler
     *
     * @see ICredentialsProvider
     */
    val connectionService: IConnectionService
        get() {
            Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization")
            return InternalOwl.getDefault().getConnectionService()
        }

    /**
     * Provides access to the interpreter service of RSSOwl. This service provides
     * API to convert a stream of data into a model representation. In the common
     * case of a XML stream this involves using a XML-Parser and creating the
     * model out of the content. Various extension points allow to customize the
     * behavor of the interpreter:
     *
     *  * Contribute a new format interpreter using the FormatInterpreter
     * extension point. This allows to display any XML in RSSOwl as Feed.
     *  * Contribute a new namespace handler using the NamespaceHandler extension
     * point. This allows to properly handle any new namespace in RSSOwl.
     *  * Contribute a new element handler using the ElementHandler extension
     * point. This makes RSSOwl understand new elements or even attributes.
     *  * Contribute a new xml parser using the XMLParser extension point if you
     * are not happy with the default one.
     *
     *
     * @return Returns the service responsible for interpreting a resource.
     * @see IFormatInterpreter
     *
     * @see IElementHandler
     *
     * @see INamespaceHandler
     *
     * @see IXMLParser
     */
    val interpreter: IInterpreterService
        get() {
            Assert.isTrue(InternalOwl.getDefault().isStarted(), "The Owl facade has not yet finished initialization")
            return InternalOwl.getDefault().getInterpreter()
        }

    /**
     * @return `true` if [Owl.startup] has
     * been called already and `false` otherwise.
     */
    val isStarted: Boolean
        get() = InternalOwl.getDefault().isStarted()

    /**
     * @return `true` if [Owl.shutdown] has been called
     * already and `false` otherwise.
     */
    val isShuttingDown: Boolean
        get() = InternalOwl.getDefault().isShuttingDown()

    /**
     * Gives extra information on the state to the
     * [Owl.startup] call
     */
    enum class StartLevel {
        NOT_STARTED, STARTING, DB_OPENED, SEARCH_INDEX_OPENED, STARTED
    }

    /**
     * Triggers the startup sequence of the Owl core. Will return immediately if
     * the core has already been started.
     *
     * @param monitor A progress monitor to report progress on long running
     * operations (e.g. migration).
     */
    fun startup(monitor: LongOperationMonitor) {
        if (!InternalOwl.getDefault().isStarted())
            InternalOwl.getDefault().startup(monitor, false, false)
    }

    /**
     * @param emergency If set to `TRUE`, this method is called from a
     * shutdown hook that got triggered from a non-normal shutdown (e.g. System
     * Shutdown).
     */
    fun shutdown(emergency: Boolean) {
        InternalOwl.getDefault().shutdown(emergency)
    }
}
