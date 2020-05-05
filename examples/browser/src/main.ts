// @ts-ignore
import {Application as App} from "stimulus";
import {DetectorController} from './controllers/detector.controller';

const app = App.start()
app.register("detector", DetectorController);
