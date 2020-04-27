{-| Agda main module.
-}
module Agda.Main where

import Control.Monad.State

import Data.Maybe

import System.Environment
import System.Console.GetOpt

import Agda.Interaction.CommandLine
import Agda.Interaction.ExitCode (AgdaError(..), exitSuccess, exitAgdaWith)
import Agda.Interaction.Options
import Agda.Interaction.Options.Help (Help (..))
import Agda.Interaction.Monad
import Agda.Interaction.EmacsTop (mimicGHCi)
import Agda.Interaction.JSONTop (jsonREPL)
import Agda.Interaction.Imports (MaybeWarnings'(..))
import Agda.Interaction.FindFile ( SourceFile(SourceFile) )
import qualified Agda.Interaction.Imports as Imp
import qualified Agda.Interaction.Highlighting.Dot as Dot
import qualified Agda.Interaction.Highlighting.LaTeX as LaTeX
import Agda.Interaction.Highlighting.HTML

import Agda.TypeChecking.Monad
import qualified Agda.TypeChecking.Monad.Benchmark as Bench
import Agda.TypeChecking.Errors
import Agda.TypeChecking.Warnings
import Agda.TypeChecking.Pretty

import Agda.Compiler.Backend
import Agda.Compiler.Builtin

import Agda.Utils.Monad
import Agda.Utils.String

import Agda.VersionCommit

import qualified Agda.Utils.Benchmark as UtilsBench
import Agda.Utils.Except ( MonadError(catchError, throwError) )
import Agda.Utils.Impossible

-- | The main function
runAgda :: [Backend] -> IO ()
runAgda backends = runAgda' $ builtinBackends ++ backends

-- | The main function without importing built-in backends
runAgda' :: [Backend] -> IO ()
runAgda' backends = runTCMPrettyErrors $ do
  progName <- liftIO getProgName
  argv     <- liftIO getArgs
  opts     <- liftIO $ runOptM $ parseBackendOptions backends argv defaultOptions
  case opts of
    Left  err        -> liftIO $ optionError err
    Right (bs, opts) -> do
      setTCLens stBackends bs
      let enabled (Backend b) = isEnabled b (options b)
          bs' = filter enabled bs
      () <$ runAgdaWithOptions backends generateHTML (interaction bs') progName opts
      where
        interaction bs = backendInteraction bs $ defaultInteraction opts

defaultInteraction :: CommandLineOptions -> TCM (Maybe Interface) -> TCM ()
defaultInteraction opts
  | i         = runIM . interactionLoop
  | ghci      = mimicGHCi . (failIfInt =<<)
  | json      = jsonREPL . (failIfInt =<<)
  | otherwise = (() <$)
  where
    i    = optInteractive     opts
    ghci = optGHCiInteraction opts
    json = optJSONInteraction opts

    failIfInt Nothing  = return ()
    failIfInt (Just _) = __IMPOSSIBLE__


-- | Run Agda with parsed command line options and with a custom HTML generator
runAgdaWithOptions
  :: [Backend]          -- ^ Backends only for printing usage and version information
  -> TCM ()             -- ^ HTML generating action
  -> (TCM (Maybe Interface) -> TCM a) -- ^ Backend interaction
  -> String             -- ^ program name
  -> CommandLineOptions -- ^ parsed command line options
  -> TCM (Maybe a)
runAgdaWithOptions backends generateHTML interaction progName opts
      | Just hp <- optShowHelp opts = Nothing <$ liftIO (printUsage backends hp)
      | optShowVersion opts         = Nothing <$ liftIO (printVersion backends)
      | isNothing (optInputFile opts)
          && not (optInteractive opts)
          && not (optGHCiInteraction opts)
          && not (optJSONInteraction opts)
                            = Nothing <$ liftIO (printUsage backends GeneralHelp)
      | otherwise           = do
          -- Main function.
          -- Bill everything to root of Benchmark trie.
          UtilsBench.setBenchmarking UtilsBench.BenchmarkOn
            -- Andreas, Nisse, 2016-10-11 AIM XXIV
            -- Turn benchmarking on provisionally, otherwise we lose track of time spent
            -- on e.g. LaTeX-code generation.
            -- Benchmarking might be turned off later by setCommandlineOptions

          Bench.billTo [] checkFile `finally_` do

            -- Print benchmarks.
            Bench.print

            -- Print accumulated statistics.
            printStatistics 1 Nothing =<< useTC lensAccumStatistics
  where
    checkFile = Just <$> do
      when (optInteractive opts) $ liftIO $ putStr splashScreen
      interaction $ do
        setCommandLineOptions opts
        hasFile <- hasInputFile
        -- Andreas, 2013-10-30 The following 'resetState' kills the
        -- verbosity options.  That does not make sense (see fail/Issue641).
        -- 'resetState' here does not seem to serve any purpose,
        -- thus, I am removing it.
        -- resetState
        if not hasFile then return Nothing else do
          let mode = if optOnlyScopeChecking opts
                     then Imp.ScopeCheck
                     else Imp.TypeCheck

          file    <- SourceFile <$> getInputFile
          (i, mw) <- Imp.typeCheckMain file mode =<< Imp.sourceInfo file

          -- An interface is only generated if the mode is
          -- Imp.TypeCheck and there are no warnings.
          result <- case (mode, mw) of
            (Imp.ScopeCheck, _)  -> return Nothing
            (_, NoWarnings)      -> return $ Just i
            (_, SomeWarnings ws) -> do
              ws' <- applyFlagsToTCWarnings ws
              case ws' of
                []   -> return Nothing
                cuws -> tcWarningsToError cuws

          reportSDoc "main" 50 $ pretty i

          whenM (optGenerateHTML <$> commandLineOptions) $
            generateHTML

          whenM (isJust . optDependencyGraph <$> commandLineOptions) $
            Dot.generateDot $ i

          whenM (optGenerateLaTeX <$> commandLineOptions) $
            LaTeX.generateLaTeX i

          -- Print accumulated warnings
          ws <- tcWarnings . classifyWarnings <$> Imp.getAllWarnings AllWarnings
          unless (null ws) $ do
            let banner = text $ "\n" ++ delimiter "All done; warnings encountered"
            reportSDoc "warning" 1 $
              vcat $ punctuate "\n" $ banner : (prettyTCM <$> ws)

          return result



-- | Print usage information.
printUsage :: [Backend] -> Help -> IO ()
printUsage backends hp = do
  progName <- getProgName
  putStr $ usage standardOptions_ progName hp
  when (hp == GeneralHelp) $ mapM_ (putStr . backendUsage) backends

backendUsage :: Backend -> String
backendUsage (Backend b) =
  usageInfo ("\n" ++ backendName b ++ " backend options") $
    map void (commandLineFlags b)

-- | Print version information.
printVersion :: [Backend] -> IO ()
printVersion backends = do
  putStrLn $ "Agda version " ++ versionWithCommitInfo
  mapM_ putStrLn
    [ "  - " ++ name ++ " backend version " ++ ver
    | Backend Backend'{ backendName = name, backendVersion = Just ver } <- backends ]

-- | What to do for bad options.
optionError :: String -> IO ()
optionError err = do
  prog <- getProgName
  putStrLn $ "Error: " ++ err ++ "\nRun '" ++ prog ++ " --help' for help on command line options."
  exitAgdaWith OptionError

-- | Run a TCM action in IO; catch and pretty print errors.
runTCMPrettyErrors :: TCM () -> IO ()
runTCMPrettyErrors tcm = do
    r <- runTCMTop $ tcm `catchError` \err -> do
      s2s <- prettyTCWarnings' =<< Imp.getAllWarningsOfTCErr err
      s1  <- prettyError err
      let ss = filter (not . null) $ s2s ++ [s1]
      unless (null s1) (liftIO $ putStr $ unlines ss)
      throwError err
    case r of
      Right _ -> exitSuccess
      Left _  -> exitAgdaWith TCMError
  `catchImpossible` \e -> do
    putStr $ show e
    exitAgdaWith ImpossibleError
