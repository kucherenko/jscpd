// ***********************************************************************
// Copyright (c) 2008-2013 Charlie Poole
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
// ***********************************************************************

using System;
using System.IO;

#if NUNIT_ENGINE
namespace NUnit.Engine.Internal
#elif NUNIT_FRAMEWORK
namespace NUnit.Framework.Internal
#else
namespace NUnit.Common
#endif
{
    /// <summary>
    /// Provides internal logging to the NUnit framework
    /// </summary>
    public class Logger : ILogger
    {
        private readonly static string TIME_FMT = "HH:mm:ss.fff";
        private readonly static string TRACE_FMT = "{0} {1,-5} [{2,2}] {3}: {4}";

        private string name;
        private string fullname;
        private InternalTraceLevel maxLevel;
        private TextWriter writer;

        /// <summary>
        /// Initializes a new instance of the <see cref="Logger"/> class.
        /// </summary>
        /// <param name="name">The name.</param>
        /// <param name="level">The log level.</param>
        /// <param name="writer">The writer where logs are sent.</param>
        public Logger(string name, InternalTraceLevel level, TextWriter writer)
        {
            this.maxLevel = level;
            this.writer = writer;
            this.fullname = this.name = name;
            int index = fullname.LastIndexOf('.');
            if (index >= 0)
                this.name = fullname.Substring(index + 1);
        }

        #region Error
        /// <summary>
        /// Logs the message at error level.
        /// </summary>
        /// <param name="message">The message.</param>
        public void Error(string message)
        {
            Log(InternalTraceLevel.Error, message);
        }

        /// <summary>
        /// Logs the message at error level.
        /// </summary>
        /// <param name="message">The message.</param>
        /// <param name="args">The message arguments.</param>
        public void Error(string message, params object[] args)
        {
            Log(InternalTraceLevel.Error, message, args);
        }

        //public void Error(string message, Exception ex)
        //{
        //    if (service.Level >= InternalTraceLevel.Error)
        //    {
        //        service.Log(InternalTraceLevel.Error, message, name, ex);
        //    }
        //}
        #endregion

        #region Warning
        /// <summary>
        /// Logs the message at warm level.
        /// </summary>
        /// <param name="message">The message.</param>
        public void Warning(string message)
        {
            Log(InternalTraceLevel.Warning, message);
        }

        /// <summary>
        /// Logs the message at warning level.
        /// </summary>
        /// <param name="message">The message.</param>
        /// <param name="args">The message arguments.</param>
        public void Warning(string message, params object[] args)
        {
            Log(InternalTraceLevel.Warning, message, args);
        }
        #endregion

        #region Info
        /// <summary>
        /// Logs the message at info level.
        /// </summary>
        /// <param name="message">The message.</param>
        public void Info(string message)
        {
            Log(InternalTraceLevel.Info, message);
        }

        /// <summary>
        /// Logs the message at info level.
        /// </summary>
        /// <param name="message">The message.</param>
        /// <param name="args">The message arguments.</param>
        public void Info(string message, params object[] args)
        {
            Log(InternalTraceLevel.Info, message, args);
        }
        #endregion

        #region Debug
        /// <summary>
        /// Logs the message at debug level.
        /// </summary>
        /// <param name="message">The message.</param>
        public void Debug(string message)
        {
            Log(InternalTraceLevel.Verbose, message);
        }

        /// <summary>
        /// Logs the message at debug level.
        /// </summary>
        /// <param name="message">The message.</param>
        /// <param name="args">The message arguments.</param>
        public void Debug(string message, params object[] args)
        {
            Log(InternalTraceLevel.Verbose, message, args);
        }
        #endregion

        #region Helper Methods
        private void Log(InternalTraceLevel level, string message)
        {
            if (writer != null && this.maxLevel >= level)
                WriteLog(level, message);
        }

        private void Log(InternalTraceLevel level, string format, params object[] args)
        {
            if (this.maxLevel >= level)
                WriteLog(level, string.Format( format, args ) );
        }

        private void WriteLog(InternalTraceLevel level, string message)
        {
            writer.WriteLine(TRACE_FMT,
                DateTime.Now.ToString(TIME_FMT),
                level == InternalTraceLevel.Verbose ? "Debug" : level.ToString(),
#if PORTABLE
                System.Environment.CurrentManagedThreadId,
#else
                System.Threading.Thread.CurrentThread.ManagedThreadId,
#endif
                name,
                message);
        }

#endregion
    }
}
