import {Application} from "stimulus";
import {DetectorController} from './controllers/detector.controller';

const app = Application.start()
app.register("detector", DetectorController);
